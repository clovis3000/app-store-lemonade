pub mod queries;
mod schema;

use anyhow::{Context, Result};
use rusqlite::ffi::sqlite3_auto_extension;
use rusqlite::Connection;
use sqlite_vec::sqlite3_vec_init;
use std::path::Path;
use std::sync::Once;

static VEC_INIT: Once = Once::new();

pub struct Database {
    pub conn: Connection,
}

impl Database {
    pub fn open(path: &str) -> Result<Self> {
        // Register sqlite-vec extension globally (once, before any connection opens)
        VEC_INIT.call_once(|| unsafe {
            type ExtensionInit = unsafe extern "C" fn(
                *mut rusqlite::ffi::sqlite3,
                *mut *mut std::os::raw::c_char,
                *const rusqlite::ffi::sqlite3_api_routines,
            ) -> std::os::raw::c_int;
            sqlite3_auto_extension(Some(std::mem::transmute::<*const (), ExtensionInit>(
                sqlite3_vec_init as *const (),
            )));
        });

        let path = Path::new(path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create DB directory: {}", parent.display()))?;
        }
        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open database: {}", path.display()))?;

        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;

        schema::run_migrations(&conn)?;

        // Verify sqlite-vec loaded
        let vec_version: String = conn
            .query_row("SELECT vec_version()", [], |row| row.get(0))
            .context("sqlite-vec extension failed to load")?;
        tracing::debug!("sqlite-vec {} loaded", vec_version);

        Ok(Self { conn })
    }
}
