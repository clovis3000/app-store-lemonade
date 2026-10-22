//! Research profiles. `prompts/` is the general profile; a tailored profile
//! is a folder in `profiles/` holding a `profile.toml` plus whichever prompt
//! files it overrides. Anything it doesn't override falls through to general,
//! so improvements to the general prompts reach every profile that kept them.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tera::Tera;

use crate::config::AppConfig;

pub const GENERAL: &str = "general";
const MANIFEST: &str = "profile.toml";

/// Files a profile may override.
pub const TAILORABLE: &[&str] = &[
    "competitive_research.tera",
    "synthesis.tera",
    "cross_app_synthesis.tera",
    "followup.tera",
    "review_analysis.tera",
    "classify.md",
    "classify.schema.json",
];

/// What `profile new` copies in for tailoring: the classifier taxonomy and the
/// research prompt are where a domain differs; the rest rarely needs to.
const COPIED_ON_CREATE: &[&str] = &["competitive_research.tera", "classify.md", "classify.schema.json"];

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub apps: AppSelection,
}

/// The apps a profile studies, used when a command is given no --app-id or --search.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppSelection {
    /// Terms matched against app names and descriptions in the local DB.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub search: Vec<String>,
    /// Exact App Store IDs, always included.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ids: Vec<String>,
    /// How many search matches to take, by rating count.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top: Option<usize>,
}

pub struct Profile {
    pub name: String,
    /// None for the general profile.
    pub dir: Option<PathBuf>,
    pub manifest: Manifest,
    general_dir: PathBuf,
    pub tera: Tera,
}

impl Profile {
    pub fn load(config: &AppConfig, name: &str) -> Result<Self> {
        let general_dir = PathBuf::from(&config.paths.prompts_dir);
        let (dir, manifest) = if name == GENERAL {
            (None, Manifest::default())
        } else {
            let dir = profiles_dir(config).join(name);
            let manifest_path = dir.join(MANIFEST);
            if !manifest_path.is_file() {
                bail!(
                    "No profile '{}' (looked for {}). `lemonade profile list` shows what exists.",
                    name,
                    manifest_path.display()
                );
            }
            let text = std::fs::read_to_string(&manifest_path)?;
            let manifest: Manifest = toml::from_str(&text)
                .with_context(|| format!("Failed to parse {}", manifest_path.display()))?;
            (Some(dir), manifest)
        };

        let mut profile = Profile { name: name.to_string(), dir, manifest, general_dir, tera: Tera::default() };
        for file in TAILORABLE.iter().filter(|f| f.ends_with(".tera")) {
            let path = profile.path_of(file);
            if path.is_file() {
                profile
                    .tera
                    .add_template_file(&path, Some(file))
                    .with_context(|| format!("Failed to load template {}", path.display()))?;
            }
        }
        Ok(profile)
    }

    /// The profile's own copy of `file` if it has one, else the general one.
    pub fn path_of(&self, file: &str) -> PathBuf {
        self.dir
            .as_ref()
            .map(|d| d.join(file))
            .filter(|p| p.is_file())
            .unwrap_or_else(|| self.general_dir.join(file))
    }

    pub fn overrides(&self) -> Vec<&'static str> {
        match &self.dir {
            Some(dir) => TAILORABLE.iter().copied().filter(|f| dir.join(f).is_file()).collect(),
            None => Vec::new(),
        }
    }

    pub fn render(&self, template: &str, context: &tera::Context) -> Result<String> {
        self.tera
            .render(template, context)
            .with_context(|| format!("Failed to render {} for profile '{}'", template, self.name))
    }

    pub fn classify_system_prompt(&self) -> Result<String> {
        let path = self.path_of("classify.md");
        std::fs::read_to_string(&path).with_context(|| format!("Failed to read {}", path.display()))
    }

    /// The per-review label schema. The classifier wraps it with a review_id
    /// and a batch array, so the profile only describes the labels.
    pub fn classify_schema(&self) -> Result<serde_json::Value> {
        let path = self.path_of("classify.schema.json");
        let text = std::fs::read_to_string(&path).with_context(|| format!("Failed to read {}", path.display()))?;
        let schema: serde_json::Value =
            serde_json::from_str(&text).with_context(|| format!("{} is not valid JSON", path.display()))?;
        let props = schema["properties"]
            .as_object()
            .with_context(|| format!("{} must be an object schema with properties", path.display()))?;
        if props.contains_key("review_id") {
            bail!("{} must not define review_id; the classifier adds it", path.display());
        }
        if schema["additionalProperties"] != false {
            bail!("{} needs \"additionalProperties\": false for strict structured output", path.display());
        }
        Ok(schema)
    }
}

pub fn profiles_dir(config: &AppConfig) -> PathBuf {
    PathBuf::from(&config.paths.profiles_dir)
}

/// Every profile with its description, general first.
pub fn list(config: &AppConfig) -> Result<Vec<(String, String)>> {
    let mut out = vec![(GENERAL.to_string(), "The shipped prompts in prompts/".to_string())];
    let dir = profiles_dir(config);
    if dir.is_dir() {
        let mut names: Vec<String> = std::fs::read_dir(&dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().join(MANIFEST).is_file())
            .filter_map(|e| e.file_name().into_string().ok())
            .collect();
        names.sort();
        for name in names {
            let description = Profile::load(config, &name).map(|p| p.manifest.description).unwrap_or_default();
            out.push((name, description));
        }
    }
    Ok(out)
}

/// Creates `profiles/<name>/` from `from`, copying in the files most worth
/// tailoring. Returns the new folder.
pub fn create(config: &AppConfig, name: &str, from: &str, description: &str, apps: AppSelection) -> Result<PathBuf> {
    validate_name(name)?;
    let source = Profile::load(config, from)?;
    let dir = profiles_dir(config).join(name);
    if dir.exists() {
        bail!("{} already exists", dir.display());
    }
    std::fs::create_dir_all(&dir)?;

    // Copy the starter set plus anything the source profile had tailored.
    let mut files: Vec<&str> = COPIED_ON_CREATE.to_vec();
    files.extend(source.overrides().into_iter().filter(|f| !COPIED_ON_CREATE.contains(f)));
    for file in files {
        std::fs::copy(source.path_of(file), dir.join(file))
            .with_context(|| format!("Failed to copy {} into {}", file, dir.display()))?;
    }

    let apps = if apps.search.is_empty() && apps.ids.is_empty() { source.manifest.apps.clone() } else { apps };
    let manifest = Manifest { description: description.to_string(), apps };
    std::fs::write(dir.join(MANIFEST), toml::to_string_pretty(&manifest)?)?;
    Ok(dir)
}

fn validate_name(name: &str) -> Result<()> {
    let ok = !name.is_empty()
        && name != GENERAL
        && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
    if !ok {
        bail!("Profile names are lowercase letters, digits, '-' and '_', and not '{GENERAL}'");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_name;

    #[test]
    fn names_are_folder_safe() {
        assert!(validate_name("habit-trackers").is_ok());
        assert!(validate_name("general").is_err());
        assert!(validate_name("../escape").is_err());
        assert!(validate_name("Has Space").is_err());
    }
}
