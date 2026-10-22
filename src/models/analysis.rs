use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct Synthesis {
    pub id: i64,
    pub name: String,
    pub profile: String,
    pub analysis_ids: Option<String>,
    pub app_ids: Option<String>,
    pub result_json: Option<String>,
    pub model: Option<String>,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CrossAppSynthesisResult {
    pub market_overview: String,
    pub universal_complaints: Vec<CrossAppTheme>,
    pub universal_strengths: Vec<CrossAppTheme>,
    pub underserved_opportunities: Vec<CrossAppOpportunity>,
    pub feature_gaps: Vec<FeatureGap>,
    pub pricing_landscape: String,
    pub recommended_positioning: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CrossAppTheme {
    pub label: String,
    pub apps_affected: Vec<String>,
    pub severity_or_importance: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CrossAppOpportunity {
    pub segment: String,
    pub apps_failing_them: Vec<String>,
    pub unmet_needs: Vec<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FeatureGap {
    pub feature: String,
    pub user_demand: String,
    pub current_landscape: String,
    pub opportunity: String,
}

#[derive(Debug, Clone)]
pub struct Analysis {
    pub id: i64,
    pub app_id: String,
    /// "research" or "followup" (older rows: the prompt name).
    pub prompt_name: Option<String>,
    pub profile: String,
    pub parent_id: Option<i64>,
    pub review_ids: Option<String>,
    pub result_json: Option<String>,
    pub model: Option<String>,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FollowupResult {
    pub question: String,
    pub answer: String,
    pub evidence: Vec<FollowupEvidence>,
    pub further_questions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FollowupEvidence {
    pub review_id: String,
    pub quote: String,
    pub relevance: String,
}
