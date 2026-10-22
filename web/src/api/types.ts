export interface Pagination {
  page: number;
  per_page: number;
  total: number;
  total_pages: number;
}

export interface PaginatedResponse<T> {
  data: T[];
  pagination: Pagination;
}

export interface AppSummary {
  app_id: string;
  name: string;
  developer: string | null;
  category: string | null;
  rating: number | null;
  rating_count: number | null;
  price: number | null;
  formatted_price: string | null;
  monetisation_model: string | null;
  last_updated: string | null;
  opportunity_score: number | null;
  demand_signal: number | null;
  dissatisfaction_signal: number | null;
  neglect_signal: number | null;
  feasibility_tier: number | null;
  recommended_action: string | null;
  review_count: number;
  starred: boolean;
}

export interface ReviewSummary {
  total: number;
  by_rating: Record<string, number>;
  by_country: Record<string, number>;
}

export interface AppDetail {
  app_id: string;
  name: string;
  developer: string | null;
  category: string | null;
  platform: string;
  rating: number | null;
  rating_count: number | null;
  price: number | null;
  formatted_price: string | null;
  monetisation_model: string | null;
  last_updated: string | null;
  original_release: string | null;
  version: string | null;
  description: string | null;
  app_store_url: string | null;
  file_size_bytes: number | null;
  opportunity_score: number | null;
  demand_signal: number | null;
  dissatisfaction_signal: number | null;
  neglect_signal: number | null;
  diagnosis: Diagnosis | null;
  review_count: number;
  starred: boolean;
  review_summary: ReviewSummary;
}

export interface Diagnosis {
  complaint_clusters?: ComplaintCluster[];
  monetisation_complaints?: MonetisationComplaints;
  positive_clusters?: PositiveCluster[];
  recency_analysis?: RecencyAnalysis;
  build_brief?: BuildBrief;
  feasibility_tier?: number;
  recommended_action?: string;
}

export interface ComplaintCluster {
  label: string;
  count: number;
  severity: string;
  representative_quotes: string[];
  requires: string;
}

export interface MonetisationComplaints {
  subscription_fatigue: boolean;
  price_too_high: boolean;
  ads_intrusive: boolean;
  paywall_frustration: boolean;
  suggested_pricing: string;
}

export interface PositiveCluster {
  label: string;
  count: number;
  representative_quotes: string[];
}

export interface RecencyAnalysis {
  recent_negative_spike: boolean;
  recent_complaints_summary: string;
  version_specific_complaints: string[];
}

export interface BuildBrief {
  one_liner: string;
  core_features: string[];
  differentiators: string[];
  skip_features: string[];
  pricing_recommendation: string;
  aso_keywords: string[];
}

export interface Review {
  review_id: string;
  app_id: string;
  platform: string;
  rating: number;
  title: string | null;
  body: string | null;
  author: string | null;
  date: string | null;
  country: string | null;
  version: string | null;
  scraped_at: string | null;
}

export interface StatsResponse {
  pipeline: PipelineFunnel;
  categories: CategoryStat[];
  score_distribution: ScoreBucket[];
  top_opportunities: AppSummary[];
}

export interface PipelineFunnel {
  total_apps: number;
  scored: number;
  diagnosed: number;
  actionable: number;
  total_reviews: number;
  starred: number;
}

export interface CategoryStat {
  category: string;
  count: number;
  avg_rating: number;
  scored_count: number;
}

export interface ScoreBucket {
  range: string;
  count: number;
}
