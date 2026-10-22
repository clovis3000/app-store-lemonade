use comfy_table::{presets::UTF8_FULL, Cell, CellAlignment, ContentArrangement, Table};

use crate::models::App;
use crate::text::truncate;

pub fn print_app_table(apps: &[App], filter: &str) {
    if apps.is_empty() {
        println!("No apps found for filter '{}'.", filter);
        return;
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic);

    let show_score = filter != "all";
    let show_diagnosis = filter == "diagnosed" || filter == "actionable";

    if show_diagnosis {
        table.set_header(vec![
            "#", "Name", "Category", "Rating", "Reviews", "Score", "Tier", "Action",
        ]);
    } else if show_score {
        table.set_header(vec![
            "#", "Name", "Category", "Rating", "Reviews", "Price", "Updated", "Score",
        ]);
    } else {
        table.set_header(vec![
            "#", "Name", "Category", "Rating", "Reviews", "Price", "Developer",
        ]);
    }

    for (i, app) in apps.iter().enumerate() {
        let rating_str = app
            .rating
            .map(|r| format!("{:.1}", r))
            .unwrap_or_else(|| "-".into());
        let count_str = app
            .rating_count
            .map(format_count)
            .unwrap_or_else(|| "-".into());

        if show_diagnosis {
            table.add_row(vec![
                Cell::new(i + 1).set_alignment(CellAlignment::Right),
                Cell::new(truncate(&app.name, 30)),
                Cell::new(app.category.as_deref().unwrap_or("-")),
                Cell::new(&rating_str).set_alignment(CellAlignment::Right),
                Cell::new(&count_str).set_alignment(CellAlignment::Right),
                Cell::new(format!("{:.2}", app.opportunity_score.unwrap_or(0.0)))
                    .set_alignment(CellAlignment::Right),
                Cell::new(
                    app.feasibility_tier
                        .map(|t| t.to_string())
                        .unwrap_or_else(|| "-".into()),
                )
                .set_alignment(CellAlignment::Right),
                Cell::new(app.recommended_action.as_deref().unwrap_or("-")),
            ]);
        } else if show_score {
            let updated = app
                .last_updated
                .as_ref()
                .map(|d| d.chars().take(10).collect::<String>())
                .unwrap_or_else(|| "-".into());

            table.add_row(vec![
                Cell::new(i + 1).set_alignment(CellAlignment::Right),
                Cell::new(truncate(&app.name, 30)),
                Cell::new(app.category.as_deref().unwrap_or("-")),
                Cell::new(&rating_str).set_alignment(CellAlignment::Right),
                Cell::new(&count_str).set_alignment(CellAlignment::Right),
                Cell::new(app.formatted_price.as_deref().unwrap_or("-")),
                Cell::new(&updated),
                Cell::new(format!("{:.2}", app.opportunity_score.unwrap_or(0.0)))
                    .set_alignment(CellAlignment::Right),
            ]);
        } else {
            table.add_row(vec![
                Cell::new(i + 1).set_alignment(CellAlignment::Right),
                Cell::new(truncate(&app.name, 30)),
                Cell::new(app.category.as_deref().unwrap_or("-")),
                Cell::new(&rating_str).set_alignment(CellAlignment::Right),
                Cell::new(&count_str).set_alignment(CellAlignment::Right),
                Cell::new(app.formatted_price.as_deref().unwrap_or("-")),
                Cell::new(truncate(app.developer.as_deref().unwrap_or("-"), 25)),
            ]);
        }
    }

    println!("{table}");
    println!("{} apps shown", apps.len());
}

fn format_count(n: i64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}
