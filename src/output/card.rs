use anyhow::{Context, Result};
use std::path::Path;

use crate::db::{queries, Database};
use crate::models::{DiagnosisResult, OpportunityCard, ScoreComponentsCard};
use crate::text::truncate;

pub fn show_card(db: &Database, app_id: &str, json: bool, export: Option<&Path>) -> Result<()> {
    let app = queries::get_app(db, app_id)?
        .with_context(|| format!("App not found: {}", app_id))?;

    let diagnosis: Option<DiagnosisResult> = app
        .diagnosis_json
        .as_ref()
        .and_then(|j| serde_json::from_str(j).ok());

    let card = OpportunityCard {
        app_id: app.app_id.clone(),
        name: app.name.clone(),
        developer: app.developer.clone(),
        category: app.category.clone(),
        platform: app.platform.clone(),
        rating: app.rating,
        rating_count: app.rating_count,
        price: app.price,
        formatted_price: app.formatted_price.clone(),
        last_updated: app.last_updated.clone(),
        opportunity_score: app.opportunity_score,
        score_components: match (app.demand_signal, app.dissatisfaction_signal, app.neglect_signal)
        {
            (Some(d), Some(di), Some(n)) => Some(ScoreComponentsCard {
                demand_signal: d,
                dissatisfaction_signal: di,
                neglect_signal: n,
            }),
            _ => None,
        },
        diagnosis: diagnosis.clone(),
        competitive_scan: diagnosis.as_ref().and_then(|d| d.competitive_scan.clone()),
    };

    if json {
        let output = serde_json::to_string_pretty(&card)?;
        println!("{}", output);

        if let Some(path) = export {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(path, &output)
                .with_context(|| format!("Failed to export to {}", path.display()))?;
            eprintln!("Exported to {}", path.display());
        }
    } else {
        print_card_pretty(&app, &diagnosis);
    }

    Ok(())
}

fn print_card_pretty(app: &crate::models::App, diagnosis: &Option<DiagnosisResult>) {
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║  OPPORTUNITY CARD                                      ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  {}  ", app.name);
    println!("║  by {}  ", app.developer.as_deref().unwrap_or("Unknown"));
    println!("╠══════════════════════════════════════════════════════════╣");
    println!(
        "  Platform:  {}  |  Category: {}",
        app.platform,
        app.category.as_deref().unwrap_or("-")
    );
    println!(
        "  Rating:    {:.1} ({} reviews)  |  Price: {}",
        app.rating.unwrap_or(0.0),
        app.rating_count.unwrap_or(0),
        app.formatted_price.as_deref().unwrap_or("-")
    );
    println!(
        "  Updated:   {}",
        app.last_updated
            .as_ref()
            .map(|d| d.chars().take(10).collect::<String>())
            .unwrap_or_else(|| "-".into())
    );
    println!(
        "  Score:     {:.2}",
        app.opportunity_score.unwrap_or(0.0)
    );

    if let (Some(d), Some(di), Some(n)) = (
        app.demand_signal,
        app.dissatisfaction_signal,
        app.neglect_signal,
    ) {
        println!(
            "    demand={:.2}  dissatisfaction={:.2}  neglect={:.2}",
            d, di, n
        );
    }

    if let Some(diag) = diagnosis {
        println!();
        println!("── COMPLAINT CLUSTERS ──────────────────────────────────");
        for cluster in &diag.complaint_clusters {
            println!(
                "  [{:>8}] {} ({}x) — requires: {}",
                cluster.severity, cluster.label, cluster.count, cluster.requires
            );
            for quote in cluster.representative_quotes.iter().take(2) {
                println!("    > \"{}\"", truncate(quote, 70));
            }
        }

        println!();
        println!("── MONETISATION COMPLAINTS ─────────────────────────────");
        let mc = &diag.monetisation_complaints;
        let mut flags = vec![];
        if mc.subscription_fatigue {
            flags.push("subscription fatigue");
        }
        if mc.price_too_high {
            flags.push("price too high");
        }
        if mc.ads_intrusive {
            flags.push("intrusive ads");
        }
        if mc.paywall_frustration {
            flags.push("paywall frustration");
        }
        if flags.is_empty() {
            println!("  None detected");
        } else {
            println!("  {}", flags.join(", "));
        }
        if let Some(pricing) = &mc.suggested_pricing {
            println!("  Suggested: {}", pricing);
        }

        println!();
        println!("── WHAT USERS LOVE ────────────────────────────────────");
        for cluster in &diag.positive_clusters {
            println!("  {} ({}x)", cluster.label, cluster.count);
        }

        println!();
        println!("── BUILD BRIEF ────────────────────────────────────────");
        let bb = &diag.build_brief;
        println!("  {}", bb.one_liner);
        println!();
        println!("  Core features:");
        for f in &bb.core_features {
            println!("    - {}", f);
        }
        println!("  Differentiators:");
        for d in &bb.differentiators {
            println!("    - {}", d);
        }
        println!("  Skip:");
        for s in &bb.skip_features {
            println!("    - {}", s);
        }
        if let Some(p) = &bb.pricing_recommendation {
            println!("  Pricing: {}", p);
        }
        println!("  ASO keywords: {}", bb.aso_keywords.join(", "));

        if let Some(scan) = &diag.competitive_scan {
            println!();
            println!("── COMPETITION ────────────────────────────────────────");
            match &scan.best_alternative {
                Some(best) => println!(
                    "  {} strong alternatives; best: {} ({:.1}★, {} ratings)",
                    scan.alternatives_count, best.name, best.rating, best.rating_count
                ),
                None => println!("  No strong alternative found: the gap looks open"),
            }
        }

        println!();
        println!(
            "  Feasibility tier: {}  |  Recommendation: {}",
            diag.feasibility_tier, diag.recommended_action
        );
    } else if app.diagnosed_at.is_none() {
        println!();
        println!("  (Not yet diagnosed — run `lemonade diagnose --app-id {}`)", app.app_id);
    }

    println!("╚══════════════════════════════════════════════════════════╝");
}
