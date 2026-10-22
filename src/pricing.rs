//! The one place model prices live: list USD per million tokens, standard tier.

/// (input, output) USD per million tokens. Dated IDs like
/// `claude-haiku-4-5-20251001` match by prefix, so more specific prefixes
/// must come first.
pub fn per_mtok(model: &str) -> Option<(f64, f64)> {
    const PRICES: &[(&str, f64, f64)] = &[
        // Anthropic
        ("claude-fable-5-1", 10.0, 50.0),
        ("claude-fable-5", 10.0, 50.0),
        ("claude-opus-5-5", 4.0, 20.0),
        ("claude-opus-5", 5.0, 25.0),
        ("claude-opus-4", 5.0, 25.0),
        ("claude-sonnet-5", 2.0, 10.0),
        ("claude-sonnet-4-6", 3.0, 15.0),
        ("claude-haiku-5-5", 0.10, 0.50),
        ("claude-haiku-4-5", 1.0, 5.0),
        // OpenAI (2026-10 rates; GPT-6 Luna was halved in October)
        ("gpt-6-luna", 0.10, 0.50),
        ("gpt-5.6-luna", 0.20, 1.20),
        ("gpt-5-mini", 0.25, 2.00),
        ("text-embedding-3-small", 0.02, 0.0),
    ];
    PRICES
        .iter()
        .find(|(prefix, _, _)| model.starts_with(prefix))
        .map(|&(_, input, output)| (input, output))
}

/// USD for a call, or None when the model's price isn't known.
pub fn cost(model: &str, input_tokens: u64, output_tokens: u64) -> Option<f64> {
    per_mtok(model).map(|(i, o)| (input_tokens as f64 * i + output_tokens as f64 * o) / 1_000_000.0)
}

#[cfg(test)]
mod tests {
    use super::per_mtok;

    #[test]
    fn matches_dated_ids_and_rejects_unknown() {
        assert_eq!(per_mtok("claude-haiku-4-5-20251001"), Some((1.0, 5.0)));
        assert_eq!(per_mtok("claude-opus-5-5"), Some((4.0, 20.0)));
        assert_eq!(per_mtok("gpt-6-luna"), Some((0.10, 0.50)));
        assert_eq!(per_mtok("gpt-6.1-sol"), None);
    }
}
