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

        // Enable WAL mode for better concurrent read performance
        conn.execute_batch("PRAGMA journal_mode=WAL;")
            .map_err(|e| AppError::Database(format!("Failed to set journal mode: {}", e)))?;

        // Enable foreign keys
        conn.execute_batch("PRAGMA foreign_keys=ON;")
            .map_err(|e| AppError::Database(format!("Failed to enable foreign keys: {}", e)))?;

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
