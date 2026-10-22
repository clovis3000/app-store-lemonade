use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Review {
    pub review_id: String,
    pub app_id: String,
    pub platform: String,
    pub rating: i32,
    pub title: Option<String>,
    pub body: Option<String>,
    pub author: Option<String>,
    pub date: Option<String>,
    pub country: Option<String>,
    pub version: Option<String>,
    pub scraped_at: Option<String>,
}

/// iTunes RSS customer reviews response
#[derive(Debug, Deserialize)]
pub struct ReviewFeedResponse {
    pub feed: ReviewFeed,
}

#[derive(Debug, Deserialize)]
pub struct ReviewFeed {
    #[serde(default, deserialize_with = "one_or_many")]
    pub entry: Option<Vec<ReviewFeedEntry>>,
}

/// The feed's `entry` is an array, except on a page holding exactly one
/// review, where Apple sends the bare object.
fn one_or_many<'de, D>(deserializer: D) -> Result<Option<Vec<ReviewFeedEntry>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        One(Box<ReviewFeedEntry>),
        Many(Vec<ReviewFeedEntry>),
    }
    Ok(match Option::<OneOrMany>::deserialize(deserializer)? {
        Some(OneOrMany::One(entry)) => Some(vec![*entry]),
        Some(OneOrMany::Many(entries)) => Some(entries),
        None => None,
    })
}

#[derive(Debug, Deserialize)]
pub struct ReviewFeedEntry {
    pub id: ReviewFeedLabel,
    pub title: ReviewFeedLabel,
    pub content: Option<ReviewFeedContent>,
    pub author: Option<ReviewFeedAuthor>,
    #[serde(rename = "im:rating")]
    pub rating: ReviewFeedLabel,
    #[serde(rename = "im:version")]
    pub version: Option<ReviewFeedLabel>,
    pub updated: Option<ReviewFeedLabel>,
}

#[derive(Debug, Deserialize)]
pub struct ReviewFeedLabel {
    pub label: String,
}

#[derive(Debug, Deserialize)]
pub struct ReviewFeedContent {
    pub label: Option<String>,
    #[serde(default)]
    pub attributes: Option<ReviewFeedContentAttrs>,
}

#[derive(Debug, Deserialize)]
pub struct ReviewFeedContentAttrs {
    #[serde(rename = "type")]
    pub content_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReviewFeedAuthor {
    pub name: ReviewFeedLabel,
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENTRY: &str = r#"{"id":{"label":"1"},"title":{"label":"t"},"im:rating":{"label":"5"},
        "content":{"label":"b","attributes":{"type":"text"}}}"#;

    #[test]
    fn entry_accepts_array_single_object_and_absence() {
        let many: ReviewFeedResponse =
            serde_json::from_str(&format!(r#"{{"feed":{{"entry":[{ENTRY},{ENTRY}]}}}}"#)).unwrap();
        assert_eq!(many.feed.entry.unwrap().len(), 2);

        let one: ReviewFeedResponse =
            serde_json::from_str(&format!(r#"{{"feed":{{"entry":{ENTRY}}}}}"#)).unwrap();
        assert_eq!(one.feed.entry.unwrap().len(), 1);

        let none: ReviewFeedResponse = serde_json::from_str(r#"{"feed":{}}"#).unwrap();
        assert!(none.feed.entry.is_none());
    }
}
