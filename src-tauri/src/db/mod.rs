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
        // The whole app shares a single Mutex<Connection>, and the scan,
        // download worker, file watcher and UI queries all contend for it.
        // These pragmas keep that contention cheap and avoid spurious
        // "database is locked" errors:
        //
        // - journal_mode=WAL   : concurrent readers alongside a single writer
        // - synchronous=NORMAL : safe under WAL, far fewer fsyncs than FULL
        // - foreign_keys=ON    : enforce ON DELETE CASCADE relationships
        // - busy_timeout=5000  : block up to 5s on a locked db instead of erroring
        // - temp_store=MEMORY  : keep temporary b-trees in RAM
        // - cache_size=-65536  : ~64 MiB page cache (negative = KiB)
        // - mmap_size          : 256 MiB memory-mapped I/O for large reads
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
