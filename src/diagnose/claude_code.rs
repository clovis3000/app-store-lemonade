//! Research calls through headless Claude Code (`claude -p`), billed to the
//! logged-in Claude subscription (Pro or Max) instead of an API key.

use anyhow::{bail, Context, Result};
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use super::llm::{LlmUsage, SYSTEM_PROMPT};
use crate::config::RoleSettings;
use crate::pricing;

const CALL_TIMEOUT: Duration = Duration::from_secs(20 * 60);

/// Claude Code bills an API key ahead of the subscription whenever one is
/// set, which would quietly charge the account this mode exists to avoid.
const API_KEY_VARS: &[&str] = &["ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN"];

fn claude() -> Command {
    let mut cmd = Command::new("claude");
    for var in API_KEY_VARS {
        cmd.env_remove(var);
    }
    cmd
}

/// Refuses unless Claude Code is installed and logged in to a claude.ai
/// subscription. Checked once per run, before any scraping.
pub async fn preflight() -> Result<()> {
    let output = claude()
        .args(["auth", "status"])
        .output()
        .await
        .context("Couldn't run `claude`. Install Claude Code, or pick another provider in lemonade.toml")?;
    let status: serde_json::Value = serde_json::from_slice(&output.stdout)
        .with_context(|| format!("Unexpected `claude auth status` output: {}", String::from_utf8_lossy(&output.stdout)))?;

    if status["loggedIn"] != true {
        bail!("Claude Code is not logged in. Run `claude` once and sign in with your Claude account");
    }
    if status["authMethod"] != "claude.ai" {
        bail!(
            "Claude Code is authenticated by {} rather than a claude.ai subscription, so calls would not be \
             billed to your plan. Log in with `claude auth login`, or use provider = \"anthropic\"",
            status["authMethod"]
        );
    }
    tracing::info!(
        "Using Claude Code on the {} subscription of {}; calls count against its usage limits",
        status["subscriptionType"].as_str().unwrap_or("Claude"),
        status["email"].as_str().unwrap_or("this account"),
    );
    Ok(())
}

/// One call. Never fails outright: errors come back in the first slot so the
/// batch can carry on, as with the API providers.
pub async fn call(settings: &RoleSettings, prompt: &str, schema: &serde_json::Value) -> (Result<serde_json::Value>, LlmUsage) {
    match run(settings, prompt, schema).await {
        Ok((value, usage)) => (value, usage),
        Err(e) => (Err(e), LlmUsage::default()),
    }
}

async fn run(
    settings: &RoleSettings,
    prompt: &str,
    schema: &serde_json::Value,
) -> Result<(Result<serde_json::Value>, LlmUsage)> {
    let mut cmd = claude();
    cmd.current_dir(quiet_dir()?)
        .args(["-p", "--output-format", "json", "--model", &settings.model])
        // A bare model: no tools, no MCP servers, no user/project settings or
        // hooks, no saved session. Not --bare itself: it ignores OAuth and so
        // can only bill an API key.
        .args(["--tools", "", "--strict-mcp-config", "--setting-sources", "", "--no-session-persistence"])
        .args(["--system-prompt", SYSTEM_PROMPT, "--json-schema", &schema.to_string()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if let Some(effort) = &settings.effort {
        cmd.args(["--effort", effort]);
    }

    let mut child = cmd.spawn().context("Failed to start `claude`")?;
    // The prompt goes over stdin: review batches are too long for argv.
    let mut stdin = child.stdin.take().context("No stdin for `claude`")?;
    stdin.write_all(prompt.as_bytes()).await?;
    drop(stdin);

    let output = tokio::time::timeout(CALL_TIMEOUT, child.wait_with_output())
        .await
        .context("`claude -p` timed out")??;
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).with_context(|| {
        format!(
            "`claude -p` exited {} without a JSON report: {}",
            output.status,
            crate::text::truncate(&String::from_utf8_lossy(&output.stderr), 300)
        )
    })?;
    Ok(parse_report(&report, &settings.model))
}

fn parse_report(report: &serde_json::Value, model: &str) -> (Result<serde_json::Value>, LlmUsage) {
    let u = &report["usage"];
    let input = ["input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens"]
        .iter()
        .filter_map(|k| u[*k].as_u64())
        .sum();
    let output = u["output_tokens"].as_u64().unwrap_or(0);
    let usage = LlmUsage {
        input_tokens: input,
        output_tokens: output,
        calls: 1,
        cost_usd: 0.0,
        notional_usd: report["total_cost_usd"]
            .as_f64()
            .or_else(|| pricing::cost(model, input, output))
            .unwrap_or(0.0),
    };

    // Usage limits, auth expiry and refusals all arrive as an error report.
    if report["is_error"] == true || report["subtype"] != "success" {
        let detail = report["result"].as_str().unwrap_or("no detail");
        return (
            Err(anyhow::anyhow!(
                "claude -p {}: {}",
                report["subtype"].as_str().unwrap_or("error"),
                crate::text::truncate(detail, 300)
            )),
            usage,
        );
    }
    let value = match &report["structured_output"] {
        serde_json::Value::Null => Err(anyhow::anyhow!("claude -p returned no structured output")),
        v => Ok(v.clone()),
    };
    (value, usage)
}

/// An empty working directory, so Claude Code finds no project CLAUDE.md to
/// prepend to every call.
fn quiet_dir() -> Result<PathBuf> {
    let dir = std::env::temp_dir().join("lemonade-claude-code");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::parse_report;

    #[test]
    fn reads_structured_output_and_counts_cached_input() {
        let report = serde_json::json!({
            "type": "result", "subtype": "success", "is_error": false,
            "structured_output": {"sum": 4},
            "total_cost_usd": 0.0012,
            "usage": {"input_tokens": 2, "cache_creation_input_tokens": 1415, "cache_read_input_tokens": 0, "output_tokens": 74}
        });
        let (value, usage) = parse_report(&report, "claude-haiku-5-5");
        assert_eq!(value.unwrap()["sum"], 4);
        assert_eq!(usage.input_tokens, 1417);
        assert_eq!(usage.cost_usd, 0.0);
        assert_eq!(usage.notional_usd, 0.0012);
    }

    #[test]
    fn error_reports_are_errors() {
        let report = serde_json::json!({
            "type": "result", "subtype": "error_during_execution", "is_error": true,
            "result": "usage limit reached", "usage": {}
        });
        assert!(parse_report(&report, "claude-haiku-5-5").0.is_err());
    }
}
