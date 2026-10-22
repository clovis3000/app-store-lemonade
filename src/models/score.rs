use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreComponents {
    pub demand_signal: f64,
    pub dissatisfaction_signal: f64,
    pub neglect_signal: f64,
    pub monetisation_bonus: f64,
    pub opportunity_score: f64,
}

impl ScoreComponents {
    pub fn compute(
        rating_count: i64,
        rating: f64,
        months_since_update: f64,
        monetisation_bonus: f64,
    ) -> Self {
        let demand_signal = (rating_count.max(1) as f64).log10();
        let dissatisfaction_signal = (4.5 - rating) / 4.5;
        let neglect_signal = (months_since_update / 12.0).min(2.0);
        let opportunity_score =
            demand_signal * dissatisfaction_signal * neglect_signal * monetisation_bonus;

        Self {
            demand_signal,
            dissatisfaction_signal,
            neglect_signal,
            monetisation_bonus,
            opportunity_score,
        }
    }
}
