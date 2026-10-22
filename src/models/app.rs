use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Platform {
    #[serde(rename = "ios")]
    Ios,
    #[serde(rename = "chrome")]
    Chrome,
}

impl Platform {
    pub fn as_str(&self) -> &str {
        match self {
            Platform::Ios => "ios",
            Platform::Chrome => "chrome",
        }
    }
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MonetisationModel {
    #[serde(rename = "free")]
    Free,
    #[serde(rename = "paid")]
    Paid,
    #[serde(rename = "freemium")]
    Freemium,
    #[serde(rename = "subscription")]
    Subscription,
    #[serde(rename = "unknown")]
    Unknown,
}

impl MonetisationModel {
    pub fn as_str(&self) -> &str {
        match self {
            MonetisationModel::Free => "free",
            MonetisationModel::Paid => "paid",
            MonetisationModel::Freemium => "freemium",
            MonetisationModel::Subscription => "subscription",
            MonetisationModel::Unknown => "unknown",
        }
    }

    pub fn from_str_lossy(s: &str) -> Self {
        match s {
            "free" => Self::Free,
            "paid" => Self::Paid,
            "freemium" => Self::Freemium,
            "subscription" => Self::Subscription,
            _ => Self::Unknown,
        }
    }

    /// Infer monetisation model from price and description text
    pub fn infer(price: f64, description: &str) -> Self {
        let desc_lower = description.to_lowercase();
        if price > 0.0 {
            if desc_lower.contains("subscri") || desc_lower.contains("premium") {
                Self::Subscription
            } else {
                Self::Paid
            }
        } else if desc_lower.contains("subscri") || desc_lower.contains("premium") {
            Self::Subscription
        } else if desc_lower.contains("in-app") || desc_lower.contains("upgrade") || desc_lower.contains("pro version") {
            Self::Freemium
        } else {
            Self::Free
        }
    }

    pub fn scoring_bonus(&self) -> f64 {
        match self {
            MonetisationModel::Paid | MonetisationModel::Subscription => 1.3,
            MonetisationModel::Freemium => 1.1,
            _ => 1.0,
        }
    }
}

/// Minimal app data from RSS feed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RssEntry {
    pub id: String,
    pub name: String,
    pub artist_name: String,
    pub artwork_url: Option<String>,
    pub genres: Vec<Genre>,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Genre {
    #[serde(rename = "genreId")]
    pub genre_id: String,
    pub name: String,
}

/// Full app data (enriched via iTunes Lookup)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct App {
    pub app_id: String,
    pub platform: String,
    pub name: String,
    pub developer: Option<String>,
    pub category: Option<String>,
    pub subcategory: Option<String>,
    pub rating: Option<f64>,
    pub rating_count: Option<i64>,
    pub price: Option<f64>,
    pub formatted_price: Option<String>,
    pub monetisation_model: Option<String>,
    pub last_updated: Option<String>,
    pub original_release: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub app_store_url: Option<String>,
    pub file_size_bytes: Option<i64>,
    pub current_version_rating: Option<f64>,
    pub scraped_at: Option<String>,
    // Computed scores
    pub demand_signal: Option<f64>,
    pub dissatisfaction_signal: Option<f64>,
    pub neglect_signal: Option<f64>,
    pub opportunity_score: Option<f64>,
    // Diagnosis
    pub diagnosis_json: Option<String>,
    pub diagnosed_at: Option<String>,
    pub feasibility_tier: Option<i32>,
    pub recommended_action: Option<String>,
    // User flags
    pub starred: bool,
}

/// iTunes Lookup API response wrapper
#[derive(Debug, Deserialize)]
pub struct LookupResponse {
    #[serde(rename = "resultCount")]
    pub result_count: i64,
    pub results: Vec<LookupResult>,
}

/// Single result from iTunes Lookup/Search API
#[derive(Debug, Clone, Deserialize)]
pub struct LookupResult {
    #[serde(rename = "trackId")]
    pub track_id: Option<i64>,
    #[serde(rename = "trackName")]
    pub track_name: Option<String>,
    #[serde(rename = "sellerName")]
    pub seller_name: Option<String>,
    #[serde(rename = "primaryGenreName")]
    pub primary_genre_name: Option<String>,
    #[serde(rename = "primaryGenreId")]
    pub primary_genre_id: Option<i64>,
    #[serde(rename = "averageUserRating")]
    pub average_user_rating: Option<f64>,
    #[serde(rename = "userRatingCount")]
    pub user_rating_count: Option<i64>,
    pub price: Option<f64>,
    #[serde(rename = "formattedPrice")]
    pub formatted_price: Option<String>,
    #[serde(rename = "currentVersionReleaseDate")]
    pub current_version_release_date: Option<String>,
    #[serde(rename = "releaseDate")]
    pub release_date: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "fileSizeBytes")]
    pub file_size_bytes: Option<String>,
    #[serde(rename = "trackViewUrl")]
    pub track_view_url: Option<String>,
    #[serde(rename = "isGameCenterEnabled")]
    pub is_game_center_enabled: Option<bool>,
    #[serde(rename = "trackContentRating")]
    pub track_content_rating: Option<String>,
    #[serde(rename = "averageUserRatingForCurrentVersion")]
    pub average_user_rating_for_current_version: Option<f64>,
}

/// Apple RSS top charts feed response
#[derive(Debug, Deserialize)]
pub struct RssFeedResponse {
    pub feed: RssFeed,
}

#[derive(Debug, Deserialize)]
pub struct RssFeed {
    pub title: Option<String>,
    pub results: Vec<RssFeedResult>,
}

#[derive(Debug, Deserialize)]
pub struct RssFeedResult {
    #[serde(rename = "artistName")]
    pub artist_name: String,
    pub id: String,
    pub name: String,
    #[serde(rename = "artworkUrl100")]
    pub artwork_url_100: Option<String>,
    pub genres: Option<Vec<Genre>>,
    pub url: String,
}
