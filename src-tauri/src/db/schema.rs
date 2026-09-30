use rusqlite::Connection;

use crate::errors::AppError;

/// SQL for creating the packages table
const CREATE_PACKAGES_TABLE: &str = "
CREATE TABLE IF NOT EXISTS packages (
    id              TEXT PRIMARY KEY,
    creator         TEXT NOT NULL,
    name            TEXT NOT NULL,
    version         INTEGER NOT NULL,
    file_path       TEXT NOT NULL UNIQUE,
    size_bytes      INTEGER NOT NULL DEFAULT 0,
    license_type    TEXT NOT NULL DEFAULT '',
    description     TEXT,
    credits         TEXT,
    instructions    TEXT,
    promotional_link TEXT,
    meta_json       TEXT,
    resource_types  TEXT NOT NULL DEFAULT '[]',
    file_created_time TEXT,
    scan_time       TEXT NOT NULL,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
";

/// SQL for creating the contents table (files inside .var packages)
const CREATE_CONTENTS_TABLE: &str = "
CREATE TABLE IF NOT EXISTS contents (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    package_id      TEXT NOT NULL,
    file_path       TEXT NOT NULL,
    resource_type   TEXT NOT NULL DEFAULT 'other',
    size_bytes      INTEGER NOT NULL DEFAULT 0,
    file_hash       TEXT,
    FOREIGN KEY (package_id) REFERENCES packages(id) ON DELETE CASCADE
);
";

/// SQL for creating the dependencies table
const CREATE_DEPENDENCIES_TABLE: &str = "
CREATE TABLE IF NOT EXISTS dependencies (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    package_id      TEXT NOT NULL,
    depends_on_id   TEXT NOT NULL,
    required_version TEXT NOT NULL DEFAULT 'latest',
    FOREIGN KEY (package_id) REFERENCES packages(id) ON DELETE CASCADE,
    UNIQUE(package_id, depends_on_id)
);
";

/// SQL for creating the scene_references table
const CREATE_SCENE_REFERENCES_TABLE: &str = "
CREATE TABLE IF NOT EXISTS scene_references (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    scene_package_id TEXT NOT NULL,
    scene_file_path TEXT NOT NULL,
    referenced_package_id TEXT NOT NULL,
    referenced_file_path  TEXT NOT NULL,
    FOREIGN KEY (scene_package_id) REFERENCES packages(id) ON DELETE CASCADE
);
";

/// SQL for creating the migration_log table
const CREATE_MIGRATION_LOG_TABLE: &str = "
CREATE TABLE IF NOT EXISTS migration_log (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    version         INTEGER NOT NULL UNIQUE,
    description     TEXT NOT NULL,
    applied_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
";

const CREATE_RESOURCE_MIGRATION_LOG_TABLE: &str = "
CREATE TABLE IF NOT EXISTS resource_migration_log (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id         TEXT NOT NULL,
    package_id      TEXT NOT NULL,
    source_path     TEXT NOT NULL,
    destination_path TEXT NOT NULL,
    action          TEXT NOT NULL,
    size_bytes      INTEGER NOT NULL DEFAULT 0,
    status          TEXT NOT NULL,
    error           TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    completed_at    TEXT,
    rolled_back_at  TEXT
);
";

const CREATE_CLEANUP_TRASH_TABLE: &str = "
CREATE TABLE IF NOT EXISTS cleanup_trash (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    package_id      TEXT NOT NULL,
    original_path   TEXT NOT NULL,
    trash_path      TEXT NOT NULL,
    size_bytes      INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    restored_at     TEXT
);
";

const CREATE_PACKAGE_TAGS_TABLE: &str = "
CREATE TABLE IF NOT EXISTS package_tags (
    package_id      TEXT NOT NULL,
    tag             TEXT NOT NULL,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (package_id, tag),
    FOREIGN KEY (package_id) REFERENCES packages(id) ON DELETE CASCADE
);
";

/// SQL for creating the physical_packages table
const CREATE_PHYSICAL_PACKAGES_TABLE: &str = "
CREATE TABLE IF NOT EXISTS physical_packages (
    file_path       TEXT PRIMARY KEY,
    package_id      TEXT NOT NULL,
    size_bytes      INTEGER NOT NULL DEFAULT 0,
    modified_time   TEXT,
    file_md5        TEXT,
    scan_time       TEXT NOT NULL,
    scan_status     TEXT NOT NULL DEFAULT 'ok',
    last_error      TEXT
);
";

const CREATE_ON_DEMAND_PLANS_TABLE: &str = "
CREATE TABLE IF NOT EXISTS on_demand_plans (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    package_ids     TEXT NOT NULL DEFAULT '[]',
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
";

const CREATE_ON_DEMAND_STATE_TABLE: &str = "
CREATE TABLE IF NOT EXISTS on_demand_state (
    key             TEXT PRIMARY KEY,
    value           TEXT NOT NULL
);
";

/// Index creation statements for query performance
const CREATE_INDEXES: &str = "
CREATE INDEX IF NOT EXISTS idx_packages_creator ON packages(creator);
CREATE INDEX IF NOT EXISTS idx_packages_name ON packages(name);
CREATE INDEX IF NOT EXISTS idx_packages_scan_time ON packages(scan_time);
CREATE INDEX IF NOT EXISTS idx_contents_package_id ON contents(package_id);
CREATE INDEX IF NOT EXISTS idx_contents_resource_type ON contents(resource_type);
CREATE INDEX IF NOT EXISTS idx_contents_file_hash ON contents(file_hash);
CREATE INDEX IF NOT EXISTS idx_dependencies_package_id ON dependencies(package_id);
CREATE INDEX IF NOT EXISTS idx_dependencies_depends_on ON dependencies(depends_on_id);
CREATE INDEX IF NOT EXISTS idx_scene_refs_scene_pkg ON scene_references(scene_package_id);
CREATE INDEX IF NOT EXISTS idx_scene_refs_ref_pkg ON scene_references(referenced_package_id);
CREATE INDEX IF NOT EXISTS idx_phys_pkg_id ON physical_packages(package_id);
CREATE INDEX IF NOT EXISTS idx_phys_pkg_md5 ON physical_packages(file_md5);
CREATE INDEX IF NOT EXISTS idx_on_demand_plans_updated ON on_demand_plans(updated_at);
CREATE INDEX IF NOT EXISTS idx_resource_migration_task ON resource_migration_log(task_id);
CREATE INDEX IF NOT EXISTS idx_cleanup_trash_restored ON cleanup_trash(restored_at);
CREATE INDEX IF NOT EXISTS idx_package_tags_tag ON package_tags(tag);
";

/// Execute all CREATE TABLE and CREATE INDEX statements
pub fn create_tables(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS game_content_names (
        vam_root TEXT NOT NULL, resource_key TEXT NOT NULL, name TEXT NOT NULL,
        PRIMARY KEY (vam_root, resource_key)
    );")?;
    conn.execute_batch(CREATE_PACKAGES_TABLE)
        .map_err(|e| AppError::Database(format!("Failed to create packages table: {}", e)))?;

    conn.execute_batch(CREATE_PHYSICAL_PACKAGES_TABLE)
        .map_err(|e| {
            AppError::Database(format!("Failed to create physical_packages table: {}", e))
        })?;
    ensure_column(conn, "physical_packages", "modified_time", "TEXT")?;
    ensure_column(
        conn,
        "physical_packages",
        "scan_status",
        "TEXT NOT NULL DEFAULT 'ok'",
    )?;
    ensure_column(conn, "physical_packages", "last_error", "TEXT")?;

    conn.execute_batch(CREATE_CONTENTS_TABLE)
        .map_err(|e| AppError::Database(format!("Failed to create contents table: {}", e)))?;

    // Perform migration if database existed before and lacks the file_hash column
    let has_file_hash = {
        let mut stmt = conn.prepare("PRAGMA table_info(contents)").map_err(|e| {
            AppError::Database(format!("Failed to prepare table info query: {}", e))
        })?;
        let mut rows = stmt
            .query([])
            .map_err(|e| AppError::Database(format!("Failed to query table info: {}", e)))?;
        let mut found = false;
        while let Some(row) = rows
            .next()
            .map_err(|e| AppError::Database(format!("Failed to retrieve table info row: {}", e)))?
        {
            let col_name: String = row
                .get(1)
                .map_err(|e| AppError::Database(format!("Failed to read column name: {}", e)))?;
            if col_name == "file_hash" {
                found = true;
                break;
            }
        }
        found
    };

    if !has_file_hash {
        conn.execute("ALTER TABLE contents ADD COLUMN file_hash TEXT;", [])
            .map_err(|e| {
                AppError::Database(format!(
                    "Failed to add file_hash column to contents table: {}",
                    e
                ))
            })?;
    }

    let has_file_created_time = {
        let mut stmt = conn.prepare("PRAGMA table_info(packages)").map_err(|e| {
            AppError::Database(format!(
                "Failed to prepare packages table info query: {}",
                e
            ))
        })?;
        let mut rows = stmt.query([]).map_err(|e| {
            AppError::Database(format!("Failed to query packages table info: {}", e))
        })?;
        let mut found = false;
        while let Some(row) = rows.next().map_err(|e| {
            AppError::Database(format!("Failed to retrieve packages table info row: {}", e))
        })? {
            let col_name: String = row.get(1).map_err(|e| {
                AppError::Database(format!("Failed to read packages column name: {}", e))
            })?;
            if col_name == "file_created_time" {
                found = true;
                break;
            }
        }
        found
    };

    if !has_file_created_time {
        conn.execute(
            "ALTER TABLE packages ADD COLUMN file_created_time TEXT;",
            [],
        )
        .map_err(|e| {
            AppError::Database(format!(
                "Failed to add file_created_time column to packages table: {}",
                e
            ))
        })?;
    }

    conn.execute_batch(CREATE_DEPENDENCIES_TABLE)
        .map_err(|e| AppError::Database(format!("Failed to create dependencies table: {}", e)))?;

    conn.execute_batch(CREATE_SCENE_REFERENCES_TABLE)
        .map_err(|e| {
            AppError::Database(format!("Failed to create scene_references table: {}", e))
        })?;

    conn.execute_batch(CREATE_MIGRATION_LOG_TABLE)
        .map_err(|e| AppError::Database(format!("Failed to create migration_log table: {}", e)))?;

    conn.execute_batch(CREATE_RESOURCE_MIGRATION_LOG_TABLE)
        .map_err(|e| {
            AppError::Database(format!(
                "Failed to create resource_migration_log table: {}",
                e
            ))
        })?;

    ensure_column(conn, "resource_migration_log", "file_modified_time", "TEXT")?;

    conn.execute_batch(CREATE_CLEANUP_TRASH_TABLE)
        .map_err(|e| AppError::Database(format!("Failed to create cleanup_trash table: {}", e)))?;

    conn.execute_batch(CREATE_PACKAGE_TAGS_TABLE)
        .map_err(|e| AppError::Database(format!("Failed to create package_tags table: {}", e)))?;

    conn.execute_batch(CREATE_ON_DEMAND_PLANS_TABLE)
        .map_err(|e| {
            AppError::Database(format!("Failed to create on_demand_plans table: {}", e))
        })?;

    conn.execute_batch(CREATE_ON_DEMAND_STATE_TABLE)
        .map_err(|e| {
            AppError::Database(format!("Failed to create on_demand_state table: {}", e))
        })?;

    conn.execute_batch(CREATE_INDEXES)
        .map_err(|e| AppError::Database(format!("Failed to create indexes: {}", e)))?;

    // Record initial migration if not already present
    conn.execute(
        "INSERT OR IGNORE INTO migration_log (version, description) VALUES (1, 'Initial schema')",
        [],
    )
    .map_err(|e| AppError::Database(format!("Failed to record migration: {}", e)))?;

    Ok(())
}

fn ensure_column(
    conn: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> Result<(), AppError> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({})", table))
        .map_err(|e| {
            AppError::Database(format!(
                "Failed to prepare {} table info query: {}",
                table, e
            ))
        })?;
    let mut rows = stmt
        .query([])
        .map_err(|e| AppError::Database(format!("Failed to query {} table info: {}", table, e)))?;

    while let Some(row) = rows.next().map_err(|e| {
        AppError::Database(format!(
            "Failed to retrieve {} table info row: {}",
            table, e
        ))
    })? {
        let col_name: String = row.get(1).map_err(|e| {
            AppError::Database(format!("Failed to read {} column name: {}", table, e))
        })?;
        if col_name == column {
            return Ok(());
        }
    }

    let sql = format!(
        "ALTER TABLE {} ADD COLUMN {} {};",
        table, column, definition
    );
    conn.execute(&sql, []).map_err(|e| {
        AppError::Database(format!(
            "Failed to add {} column to {} table: {}",
            column, table, e
        ))
    })?;
    Ok(())
}
