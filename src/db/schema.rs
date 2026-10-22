use anyhow::Result;
use rusqlite::Connection;

pub fn run_migrations(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS apps (
            app_id TEXT PRIMARY KEY,
            platform TEXT NOT NULL DEFAULT 'ios',
            name TEXT NOT NULL,
            developer TEXT,
            category TEXT,
            subcategory TEXT,
            rating REAL,
            rating_count INTEGER,
            price REAL,
            formatted_price TEXT,
            monetisation_model TEXT,
            last_updated TEXT,
            original_release TEXT,
            version TEXT,
            description TEXT,
            app_store_url TEXT,
            file_size_bytes INTEGER,
            current_version_rating REAL,
            scraped_at TEXT,
            -- Computed scores
            demand_signal REAL,
            dissatisfaction_signal REAL,
            neglect_signal REAL,
            opportunity_score REAL,
            -- Diagnosis
            diagnosis_json TEXT,
            diagnosed_at TEXT,
            feasibility_tier INTEGER,
            recommended_action TEXT
        );

        CREATE TABLE IF NOT EXISTS reviews (
            review_id TEXT PRIMARY KEY,
            app_id TEXT NOT NULL,
            platform TEXT NOT NULL DEFAULT 'ios',
            rating INTEGER,
            title TEXT,
            body TEXT,
            author TEXT,
            date TEXT,
            country TEXT,
            version TEXT,
            scraped_at TEXT,
            FOREIGN KEY (app_id) REFERENCES apps(app_id)
        );

        CREATE TABLE IF NOT EXISTS scrape_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            app_id TEXT NOT NULL,
            source TEXT NOT NULL,
            country TEXT,
            scraped_at TEXT NOT NULL,
            review_count INTEGER DEFAULT 0,
            success INTEGER DEFAULT 1,
            error_message TEXT,
            FOREIGN KEY (app_id) REFERENCES apps(app_id)
        );

        CREATE INDEX IF NOT EXISTS idx_apps_score ON apps(opportunity_score DESC);
        CREATE INDEX IF NOT EXISTS idx_apps_platform ON apps(platform);
        CREATE INDEX IF NOT EXISTS idx_apps_category ON apps(category);
        CREATE INDEX IF NOT EXISTS idx_reviews_app ON reviews(app_id);
        CREATE INDEX IF NOT EXISTS idx_reviews_rating ON reviews(rating);
        CREATE INDEX IF NOT EXISTS idx_reviews_date ON reviews(date DESC);
        CREATE INDEX IF NOT EXISTS idx_scrape_log_app ON scrape_log(app_id, source, country);

        CREATE TABLE IF NOT EXISTS analyses (
            id INTEGER PRIMARY KEY,
            app_id TEXT NOT NULL,
            prompt_name TEXT,
            parent_id INTEGER,
            review_ids TEXT,
            result_json TEXT,
            model TEXT,
            created_at TEXT,
            FOREIGN KEY (app_id) REFERENCES apps(app_id),
            FOREIGN KEY (parent_id) REFERENCES analyses(id)
        );

        CREATE INDEX IF NOT EXISTS idx_analyses_app ON analyses(app_id);
        CREATE INDEX IF NOT EXISTS idx_analyses_parent ON analyses(parent_id);

        CREATE TABLE IF NOT EXISTS syntheses (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            analysis_ids TEXT,
            app_ids TEXT,
            result_json TEXT,
            model TEXT,
            created_at TEXT
        );
        ",
    )?;

    // Additive column migrations. SQLite has no ADD COLUMN IF NOT EXISTS, so
    // check first: swallowing every ALTER error also hid real failures.
    add_column(conn, "apps", "starred", "INTEGER NOT NULL DEFAULT 0")?;
    add_column(conn, "analyses", "profile", "TEXT NOT NULL DEFAULT 'general'")?;
    add_column(conn, "syntheses", "profile", "TEXT NOT NULL DEFAULT 'general'")?;

    conn.execute_batch(
        "
        -- One row per review per profile: each profile classifies with its own
        -- taxonomy, so labels are JSON matching that profile's classify.schema.json.
        CREATE TABLE IF NOT EXISTS review_labels (
            review_id TEXT NOT NULL,
            profile TEXT NOT NULL,
            labels_json TEXT NOT NULL,
            model TEXT,
            classified_at TEXT NOT NULL,
            PRIMARY KEY (review_id, profile),
            FOREIGN KEY (review_id) REFERENCES reviews(review_id)
        );
        CREATE INDEX IF NOT EXISTS idx_review_labels_profile ON review_labels(profile);

        -- FTS5 keyword search, kept in step with reviews on insert.
        CREATE VIRTUAL TABLE IF NOT EXISTS reviews_fts USING fts5(
            review_id UNINDEXED,
            title,
            body
        );

        -- sqlite-vec embeddings, shared by every profile.
        CREATE VIRTUAL TABLE IF NOT EXISTS review_embeddings USING vec0(
            review_id TEXT PRIMARY KEY,
            embedding float[1536] distance_metric=cosine
        );
        ",
    )?;

    Ok(())
}

fn add_column(conn: &Connection, table: &str, column: &str, decl: &str) -> Result<()> {
    let exists: bool = conn.query_row(
        &format!("SELECT COUNT(*) > 0 FROM pragma_table_info('{table}') WHERE name = ?1"),
        [column],
        |row| row.get(0),
    )?;
    if !exists {
        conn.execute(&format!("ALTER TABLE {table} ADD COLUMN {column} {decl}"), [])?;
    }
    Ok(())
}
