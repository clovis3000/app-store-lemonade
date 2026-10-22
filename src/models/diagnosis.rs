use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DiagnosisResult {
    pub complaint_clusters: Vec<ComplaintCluster>,
    pub monetisation_complaints: MonetisationComplaints,
    pub positive_clusters: Vec<PositiveCluster>,
    pub recency_analysis: RecencyAnalysis,
    pub build_brief: BuildBrief,
    pub feasibility_tier: i32,
    pub recommended_action: String,
    /// Filled in after the LLM call from an App Store search for strong alternatives.
    #[schemars(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub competitive_scan: Option<CompetitiveScan>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ComplaintCluster {
    pub label: String,
    pub count: i32,
    pub severity: String,
    pub representative_quotes: Vec<String>,
    pub requires: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MonetisationComplaints {
    pub subscription_fatigue: bool,
    pub price_too_high: bool,
    pub ads_intrusive: bool,
    pub paywall_frustration: bool,
    pub suggested_pricing: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PositiveCluster {
    pub label: String,
    pub count: i32,
    pub representative_quotes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RecencyAnalysis {
    pub recent_negative_spike: bool,
    pub recent_complaints_summary: Option<String>,
    pub version_specific_complaints: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BuildBrief {
    pub one_liner: String,
    pub core_features: Vec<String>,
    pub differentiators: Vec<String>,
    pub skip_features: Vec<String>,
    pub pricing_recommendation: Option<String>,
    pub aso_keywords: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CompetitiveScan {
    pub gap_open: bool,
    pub alternatives_count: i32,
    pub best_alternative: Option<CompetitiveAlternative>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CompetitiveAlternative {
    pub name: String,
    pub app_id: String,
    pub rating: f64,
    pub rating_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OpportunityCard {
    pub app_id: String,
    pub name: String,
    pub developer: Option<String>,
    pub category: Option<String>,
    pub platform: String,
    pub rating: Option<f64>,
    pub rating_count: Option<i64>,
    pub price: Option<f64>,
    pub formatted_price: Option<String>,
    pub last_updated: Option<String>,
    pub opportunity_score: Option<f64>,
    pub score_components: Option<ScoreComponentsCard>,
    pub diagnosis: Option<DiagnosisResult>,
    pub competitive_scan: Option<CompetitiveScan>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ScoreComponentsCard {
    pub demand_signal: f64,
    pub dissatisfaction_signal: f64,
    pub neglect_signal: f64,
}

// --- Competitive Research types ---

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CompetitiveResearchResult {
    pub praised_features: Vec<PraisedFeature>,
    pub hated_features: Vec<HatedFeature>,
    pub wishlist: Vec<WishlistItem>,
    pub premium_sentiment: PremiumSentiment,
    pub switching_signals: SwitchingSignals,
    pub underserved_segments: Vec<UnderservedSegment>,
    pub overall_summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PraisedFeature {
    pub label: String,
    pub count: i32,
    pub importance: String,
    #[serde(default)]
    pub review_ids: Vec<String>,
    pub representative_quotes: Vec<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HatedFeature {
    pub label: String,
    pub count: i32,
    pub severity: String,
    #[serde(default)]
    pub review_ids: Vec<String>,
    pub representative_quotes: Vec<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WishlistItem {
    pub label: String,
    pub count: i32,
    pub feasibility: String,
    #[serde(default)]
    pub review_ids: Vec<String>,
    pub representative_quotes: Vec<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PremiumSentiment {
    pub what_makes_users_pay: Vec<String>,
    pub what_makes_users_cancel: Vec<String>,
    pub price_perception: String,
    pub free_vs_paid_friction: String,
    #[serde(default)]
    pub review_ids: Vec<String>,
    pub representative_quotes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SwitchingSignals {
    pub switching_from: Vec<String>,
    pub switching_to: Vec<String>,
    pub loyalty_drivers: Vec<String>,
    #[serde(default)]
    pub review_ids: Vec<String>,
    pub representative_quotes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UnderservedSegment {
    pub segment: String,
    pub count: i32,
    #[serde(default)]
    pub review_ids: Vec<String>,
    pub complaints: Vec<String>,
    pub representative_quotes: Vec<String>,
}

impl CompetitiveResearchResult {
    /// Merge multiple batch results into one combined result.
    /// Concatenates all lists and deduplicates string vectors.
    pub fn merge(results: Vec<Self>) -> Self {
        let mut praised: Vec<PraisedFeature> = Vec::new();
        let mut hated: Vec<HatedFeature> = Vec::new();
        let mut wishlist: Vec<WishlistItem> = Vec::new();
        let mut underserved: Vec<UnderservedSegment> = Vec::new();
        let mut pay: Vec<String> = Vec::new();
        let mut cancel: Vec<String> = Vec::new();
        let mut price_perceptions: Vec<String> = Vec::new();
        let mut friction: Vec<String> = Vec::new();
        let mut premium_quotes: Vec<String> = Vec::new();
        let mut premium_ids: Vec<String> = Vec::new();
        let mut switch_from: Vec<String> = Vec::new();
        let mut switch_to: Vec<String> = Vec::new();
        let mut loyalty: Vec<String> = Vec::new();
        let mut switch_quotes: Vec<String> = Vec::new();
        let mut switch_ids: Vec<String> = Vec::new();
        let mut summaries: Vec<String> = Vec::new();

        for r in results {
            praised.extend(r.praised_features);
            hated.extend(r.hated_features);
            wishlist.extend(r.wishlist);
            underserved.extend(r.underserved_segments);
            pay.extend(r.premium_sentiment.what_makes_users_pay);
            cancel.extend(r.premium_sentiment.what_makes_users_cancel);
            price_perceptions.push(r.premium_sentiment.price_perception);
            friction.push(r.premium_sentiment.free_vs_paid_friction);
            premium_quotes.extend(r.premium_sentiment.representative_quotes);
            premium_ids.extend(r.premium_sentiment.review_ids);
            switch_from.extend(r.switching_signals.switching_from);
            switch_to.extend(r.switching_signals.switching_to);
            loyalty.extend(r.switching_signals.loyalty_drivers);
            switch_quotes.extend(r.switching_signals.representative_quotes);
            switch_ids.extend(r.switching_signals.review_ids);
            summaries.push(r.overall_summary);
        }

        fn dedup(v: &mut Vec<String>) {
            v.sort();
            v.dedup();
        }

        dedup(&mut pay);
        dedup(&mut cancel);
        dedup(&mut premium_quotes);
        dedup(&mut premium_ids);
        dedup(&mut switch_from);
        dedup(&mut switch_to);
        dedup(&mut loyalty);
        dedup(&mut switch_quotes);
        dedup(&mut switch_ids);

        Self {
            praised_features: praised,
            hated_features: hated,
            wishlist,
            underserved_segments: underserved,
            premium_sentiment: PremiumSentiment {
                what_makes_users_pay: pay,
                what_makes_users_cancel: cancel,
                price_perception: price_perceptions.join(" | "),
                free_vs_paid_friction: friction.join(" | "),
                review_ids: premium_ids,
                representative_quotes: premium_quotes,
            },
            switching_signals: SwitchingSignals {
                switching_from: switch_from,
                switching_to: switch_to,
                loyalty_drivers: loyalty,
                review_ids: switch_ids,
                representative_quotes: switch_quotes,
            },
            overall_summary: summaries.join("\n\n"),
        }
    }
}
