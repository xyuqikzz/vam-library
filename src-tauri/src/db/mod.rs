pub mod schema;

use std::path::Path;
use std::sync::Mutex;

use rusqlite::Connection;

use crate::errors::AppError;

/// Application database wrapper with thread-safe connection access
pub struct Database {
    conn: Mutex<Connection>,
}

#[allow(dead_code)]
impl Database {
    /// Open (or create) the database at the given path and run migrations
    pub fn new(db_path: &Path) -> Result<Self, AppError> {
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                AppError::Database(format!("Failed to create database directory: {}", e))
            })?;
        }

        let conn = Connection::open(db_path)
            .map_err(|e| AppError::Database(format!("Failed to open database: {}", e)))?;

        // Performance & concurrency tuning.
        //
        // The whole app shares a single Mutex<Connection>, and several
        // background workers (scanner, download worker, file watcher) compete
        // with UI read queries. WAL allows concurrent readers alongside a
        // single writer, NORMAL synchronous is safe under WAL and far faster,
        // and busy_timeout prevents transient lock contention from surfacing as
        // immediate "database is locked" errors. The cache/mmap/temp settings
        // speed up bulk scan writes and large-library queries.
        //
        // journal_mode=WAL returns a result row, so PRAGMAs are executed as a
        // batch via execute_batch.
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;\n\
             PRAGMA synchronous=NORMAL;\n\
             PRAGMA foreign_keys=ON;\n\
             PRAGMA busy_timeout=5000;\n\
             PRAGMA temp_store=MEMORY;\n\
             PRAGMA cache_size=-65536;\n\
             PRAGMA mmap_size=268435456;",
        )
        .map_err(|e| {
            AppError::Database(format!("Failed to configure database pragmas: {}", e))
        })?;

        // Run migrations
        schema::create_tables(&conn)?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Execute a closure with a reference to the database connection.
    /// This acquires the mutex lock for the duration of the closure.
    pub fn with_conn<F, T>(&self, f: F) -> Result<T, AppError>
    where
        F: FnOnce(&Connection) -> Result<T, AppError>,
    {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AppError::Database(format!("Failed to acquire database lock: {}", e)))?;
        f(&conn)
    }
}

/// Initialize the database with the default app data path
pub fn init_db(app_data_dir: &Path) -> Result<Database, AppError> {
    let db_path = app_data_dir.join("vamlibrary.db");
    Database::new(&db_path)
}
