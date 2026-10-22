use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub struct ResponseCache {
    dir: PathBuf,
}

#[derive(Serialize, Deserialize)]
struct CacheMeta {
    fetched_at: String,
    url: Option<String>,
}

impl ResponseCache {
    pub fn new(dir: &str) -> Result<Self> {
        let dir = PathBuf::from(dir);
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("Failed to create cache dir: {}", dir.display()))?;
        Ok(Self { dir })
    }

    fn key_to_path(&self, key: &str) -> PathBuf {
        // Sanitise key for filesystem
        let safe_key: String = key
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect();
        self.dir.join(safe_key)
    }

    pub fn get(&self, key: &str, max_age_hours: u64) -> Result<Option<String>> {
        let data_path = self.key_to_path(key).with_extension("json");
        let meta_path = self.key_to_path(key).with_extension("meta");

        if !data_path.exists() || !meta_path.exists() {
            return Ok(None);
        }

        // Check age
        let meta_str = std::fs::read_to_string(&meta_path)?;
        let meta: CacheMeta = serde_json::from_str(&meta_str)?;

        if let Ok(fetched) = chrono::DateTime::parse_from_rfc3339(&meta.fetched_at) {
            let age = chrono::Utc::now().signed_duration_since(fetched);
            if age.num_hours() > max_age_hours as i64 {
                return Ok(None);
            }
        }

        let data = std::fs::read_to_string(&data_path)?;
        Ok(Some(data))
    }

    pub fn put(&self, key: &str, data: &str) -> Result<()> {
        let data_path = self.key_to_path(key).with_extension("json");
        let meta_path = self.key_to_path(key).with_extension("meta");

        std::fs::write(&data_path, data)
            .with_context(|| format!("Failed to write cache: {}", data_path.display()))?;

        let meta = CacheMeta {
            fetched_at: chrono::Utc::now().to_rfc3339(),
            url: None,
        };
        let meta_str = serde_json::to_string(&meta)?;
        std::fs::write(&meta_path, meta_str)?;

        Ok(())
    }
}
