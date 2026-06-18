use rusqlite::{params, Connection};

use crate::error::{AppError, AppResult};

const CURRENT_SCHEMA_VERSION: i64 = 5;

const LEGACY_DEFAULT_PROJECT_SYMLINK_IGNORED_DIRS: &str = ".git\n.venv\nnode_modules";
const NEW_DEFAULT_PROJECT_SYMLINK_IGNORED_DIRS: &str = ".git\n.venv\nnode_modules\ntarget\ndist\nbuild\nout\n__pycache__\n.pytest_cache\n.mypy_cache\n.ruff_cache\n.next\n.nuxt\n.turbo\n.svelte-kit\n.gradle\n.idea\ncoverage\n.tox\n.cache";
const DEFAULT_PROJECT_SYMLINK_MAX_DEPTH: &str = "3";

pub fn migrate(conn: &Connection) -> AppResult<()> {
    let current = current_version(conn)?;

    if current > CURRENT_SCHEMA_VERSION {
        return Err(AppError::Database(format!(
            "database schema version {current} is newer than this app supports"
        )));
    }

    if current == 0 {
        migrate_to_v1(conn)?;
    } else {
        if current < 2 {
            migrate_to_v2(conn)?;
        }
        if current < 3 {
            migrate_to_v3(conn)?;
        }
        if current < 4 {
            migrate_to_v4(conn)?;
        }
        if current < 5 {
            migrate_to_v5(conn)?;
        }
    }

    Ok(())
}

fn current_version(conn: &Connection) -> AppResult<i64> {
    let has_table: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'schema_version'",
        [],
        |row| row.get(0),
    )?;

    if has_table == 0 {
        return Ok(0);
    }

    Ok(
        conn.query_row("SELECT version FROM schema_version LIMIT 1", [], |row| {
            row.get(0)
        })?,
    )
}

fn migrate_to_v1(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(&format!(
        r#"
        BEGIN;

        CREATE TABLE schema_version (
            version INTEGER NOT NULL
        );
        INSERT INTO schema_version (version) VALUES (5);

        CREATE TABLE projects (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            key TEXT NOT NULL UNIQUE,
            path TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'active'
                CHECK (status IN ('active', 'stale', 'hidden')),
            sessions_dir TEXT NOT NULL DEFAULT '__sessions',
            sort_index INTEGER,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );

        CREATE TABLE git_base_folders (
            id TEXT PRIMARY KEY,
            path TEXT NOT NULL UNIQUE,
            added_at INTEGER NOT NULL
        );

        CREATE TABLE skills (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            scope TEXT NOT NULL CHECK (scope IN ('global', 'project')),
            project_id TEXT,
            description TEXT,
            canonical_path TEXT NOT NULL,
            disabled INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
        );

        CREATE TABLE skill_distributions (
            skill_id TEXT NOT NULL,
            agent TEXT NOT NULL,
            role TEXT NOT NULL CHECK (role IN ('source', 'target', 'none')),
            target_path TEXT,
            CHECK (
                (role = 'target' AND target_path IS NOT NULL)
                OR
                (role IN ('source', 'none') AND target_path IS NULL)
            ),
            PRIMARY KEY (skill_id, agent),
            FOREIGN KEY (skill_id) REFERENCES skills(id) ON DELETE CASCADE
        );

        CREATE TABLE prompts (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            canonical_path TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );

        CREATE TABLE prompt_distributions (
            prompt_id TEXT NOT NULL,
            agent TEXT NOT NULL,
            role TEXT NOT NULL CHECK (role IN ('source', 'target', 'none')),
            target_path TEXT,
            CHECK (
                (role = 'target' AND target_path IS NOT NULL)
                OR
                (role IN ('source', 'none') AND target_path IS NULL)
            ),
            PRIMARY KEY (prompt_id, agent),
            FOREIGN KEY (prompt_id) REFERENCES prompts(id) ON DELETE CASCADE
        );

        CREATE TABLE providers (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            plan TEXT,
            status TEXT NOT NULL CHECK (status IN ('available', 'expired', 'failed', 'nocreds')),
            credential_source TEXT,
            connection_params TEXT,
            is_agent INTEGER NOT NULL DEFAULT 0,
            sort_index INTEGER,
            card_visible INTEGER NOT NULL DEFAULT 1,
            tray_visible INTEGER NOT NULL DEFAULT 1,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );

        CREATE TABLE provider_windows (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            provider_id TEXT NOT NULL,
            label TEXT NOT NULL,
            used_percent INTEGER NOT NULL,
            reset_label TEXT,
            FOREIGN KEY (provider_id) REFERENCES providers(id) ON DELETE CASCADE
        );

        CREATE TABLE session_index (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL,
            title TEXT NOT NULL,
            file_path TEXT NOT NULL,
            excerpt TEXT,
            source TEXT NOT NULL CHECK (source IN ('local', 'cloud', 'both')),
            size_bytes INTEGER,
            updated_at INTEGER NOT NULL,
            FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
        );

        CREATE VIRTUAL TABLE session_fts USING fts5(
            title, excerpt,
            content=session_index,
            content_rowid=rowid
        );

        CREATE TABLE task_groups (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            sort_index INTEGER,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );

        CREATE TABLE tasks (
            id TEXT PRIMARY KEY,
            group_id TEXT NOT NULL,
            direction TEXT NOT NULL
                CHECK (direction IN ('Distribution', 'Push', 'Pull')),
            action TEXT NOT NULL CHECK (action IN ('Symlink', 'Copy')),
            source_type TEXT NOT NULL CHECK (source_type IN ('Local', 'Cloud')),
            source TEXT NOT NULL,
            target_type TEXT NOT NULL CHECK (target_type IN ('Local', 'Cloud')),
            target TEXT NOT NULL,
            schedule TEXT NOT NULL DEFAULT 'manual',
            sort_index INTEGER,
            last_run_at INTEGER,
            last_status TEXT CHECK (last_status IN ('ok', 'failed', 'never') OR last_status IS NULL),
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            FOREIGN KEY (group_id) REFERENCES task_groups(id) ON DELETE CASCADE
        );

        CREATE TABLE settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE INDEX idx_skills_scope ON skills(scope);
        CREATE INDEX idx_skills_project ON skills(project_id) WHERE project_id IS NOT NULL;
        CREATE UNIQUE INDEX idx_skill_distributions_one_source
            ON skill_distributions(skill_id)
            WHERE role = 'source';
        CREATE UNIQUE INDEX idx_prompt_distributions_one_source
            ON prompt_distributions(prompt_id)
            WHERE role = 'source';
        CREATE INDEX idx_session_index_project ON session_index(project_id);
        CREATE INDEX idx_session_index_source ON session_index(source);
        CREATE INDEX idx_tasks_group ON tasks(group_id);
        CREATE INDEX idx_provider_windows_provider ON provider_windows(provider_id);

        INSERT INTO settings (key, value) VALUES ('tray_metric_mode', 'Remaining');
        INSERT INTO settings (key, value) VALUES ('webdav_url', '');
        INSERT INTO settings (key, value) VALUES ('webdav_user', '');
        INSERT INTO settings (key, value) VALUES ('webdav_pass', '');
        INSERT INTO settings (key, value) VALUES ('webdav_remote_root', 'agent-nexus-sync');
        INSERT INTO settings (key, value)
        VALUES ('sync_project_symlink_ignored_dirs', '{new_default_ignored_dirs}');
        INSERT INTO settings (key, value)
        VALUES ('sync_project_symlink_max_depth', '{default_max_depth}');

        COMMIT;
        "#,
        new_default_ignored_dirs = NEW_DEFAULT_PROJECT_SYMLINK_IGNORED_DIRS,
        default_max_depth = DEFAULT_PROJECT_SYMLINK_MAX_DEPTH,
    ))
    .or_else(|error| {
        let _ = conn.execute("ROLLBACK", params![]);
        Err(error)
    })?;

    Ok(())
}

fn migrate_to_v2(conn: &Connection) -> AppResult<()> {
    conn.execute_batch("BEGIN;").or_else(|error| {
        let _ = conn.execute("ROLLBACK", params![]);
        Err(error)
    })?;

    let result = (|| -> AppResult<()> {
        add_column_if_missing(conn, "source_type", "TEXT NOT NULL DEFAULT 'Local'")?;
        add_column_if_missing(conn, "target_type", "TEXT NOT NULL DEFAULT 'Local'")?;
        add_column_if_missing(conn, "schedule", "TEXT NOT NULL DEFAULT 'manual'")?;
        add_column_if_missing(conn, "last_run_at", "INTEGER")?;
        add_column_if_missing(conn, "last_status", "TEXT")?;
        conn.execute("UPDATE schema_version SET version = 2", [])?;
        Ok(())
    })();

    match result {
        Ok(()) => {
            conn.execute_batch("COMMIT;")?;
            Ok(())
        }
        Err(error) => {
            let _ = conn.execute("ROLLBACK", params![]);
            Err(error)
        }
    }
}

fn migrate_to_v3(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(
        r#"
        BEGIN;
        DROP TABLE IF EXISTS tasks;
        CREATE TABLE tasks (
            id TEXT PRIMARY KEY,
            group_id TEXT NOT NULL,
            direction TEXT NOT NULL
                CHECK (direction IN ('Distribution', 'Push', 'Pull')),
            action TEXT NOT NULL CHECK (action IN ('Symlink', 'Copy')),
            source_type TEXT NOT NULL CHECK (source_type IN ('Local', 'Cloud')),
            source TEXT NOT NULL,
            target_type TEXT NOT NULL CHECK (target_type IN ('Local', 'Cloud')),
            target TEXT NOT NULL,
            schedule TEXT NOT NULL DEFAULT 'manual',
            sort_index INTEGER,
            last_run_at INTEGER,
            last_status TEXT CHECK (last_status IN ('ok', 'failed', 'never') OR last_status IS NULL),
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            FOREIGN KEY (group_id) REFERENCES task_groups(id) ON DELETE CASCADE
        );
        DROP INDEX IF EXISTS idx_tasks_group;
        CREATE INDEX idx_tasks_group ON tasks(group_id);
        UPDATE schema_version SET version = 3;
        COMMIT;
        "#,
    )
    .or_else(|error| {
        let _ = conn.execute("ROLLBACK", params![]);
        Err(error)
    })?;

    Ok(())
}

fn migrate_to_v4(conn: &Connection) -> AppResult<()> {
    conn.execute_batch("BEGIN;").or_else(|error| {
        let _ = conn.execute("ROLLBACK", params![]);
        Err(error)
    })?;

    let result = (|| -> AppResult<()> {
        conn.execute(
            r#"
            INSERT INTO settings (key, value)
            VALUES ('webdav_remote_root', 'agent-nexus-sync')
            ON CONFLICT(key) DO NOTHING
            "#,
            [],
        )?;
        conn.execute("UPDATE schema_version SET version = 4", [])?;
        Ok(())
    })();

    match result {
        Ok(()) => {
            conn.execute_batch("COMMIT;")?;
            Ok(())
        }
        Err(error) => {
            let _ = conn.execute("ROLLBACK", params![]);
            Err(error)
        }
    }
}

fn migrate_to_v5(conn: &Connection) -> AppResult<()> {
    conn.execute_batch("BEGIN;").or_else(|error| {
        let _ = conn.execute("ROLLBACK", params![]);
        Err(error)
    })?;

    let result = (|| -> AppResult<()> {
        conn.execute(
            "UPDATE settings SET value = ?1 \
             WHERE key = 'sync_project_symlink_ignored_dirs' AND value = ?2",
            params![
                NEW_DEFAULT_PROJECT_SYMLINK_IGNORED_DIRS,
                LEGACY_DEFAULT_PROJECT_SYMLINK_IGNORED_DIRS
            ],
        )?;
        conn.execute(
            "INSERT INTO settings (key, value) \
             VALUES ('sync_project_symlink_max_depth', ?1) \
             ON CONFLICT(key) DO NOTHING",
            params![DEFAULT_PROJECT_SYMLINK_MAX_DEPTH],
        )?;
        conn.execute("UPDATE schema_version SET version = 5", [])?;
        Ok(())
    })();

    match result {
        Ok(()) => {
            conn.execute_batch("COMMIT;")?;
            Ok(())
        }
        Err(error) => {
            let _ = conn.execute("ROLLBACK", params![]);
            Err(error)
        }
    }
}

fn add_column_if_missing(conn: &Connection, column: &str, definition: &str) -> AppResult<()> {
    if task_column_exists(conn, column)? {
        return Ok(());
    }

    conn.execute_batch(&format!(
        "ALTER TABLE tasks ADD COLUMN {column} {definition};"
    ))?;
    Ok(())
}

fn task_column_exists(conn: &Connection, column: &str) -> AppResult<bool> {
    let mut stmt = conn.prepare("PRAGMA table_info(tasks)")?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;

    for row in rows {
        if row? == column {
            return Ok(true);
        }
    }

    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn seed_minimal_v4_settings(conn: &Connection, ignored_dirs_value: &str) {
        conn.execute(
            "CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
            [],
        )
        .expect("create settings table");
        conn.execute(
            "CREATE TABLE schema_version (version INTEGER NOT NULL)",
            [],
        )
        .expect("create schema_version table");
        conn.execute("INSERT INTO schema_version (version) VALUES (4)", [])
            .expect("seed schema_version 4");
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('sync_project_symlink_ignored_dirs', ?1)",
            [ignored_dirs_value],
        )
        .expect("seed ignored dirs");
    }

    #[test]
    fn upgrades_legacy_default_ignored_dirs_to_new_default() {
        let conn = Connection::open_in_memory().expect("open in-memory connection");
        seed_minimal_v4_settings(&conn, LEGACY_DEFAULT_PROJECT_SYMLINK_IGNORED_DIRS);
        migrate_to_v5(&conn).expect("migrate to v5");

        let value: String = conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'sync_project_symlink_ignored_dirs'",
                [],
                |row| row.get(0),
            )
            .expect("read ignored dirs");
        assert_eq!(value, NEW_DEFAULT_PROJECT_SYMLINK_IGNORED_DIRS);

        let depth: String = conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'sync_project_symlink_max_depth'",
                [],
                |row| row.get(0),
            )
            .expect("read max depth");
        assert_eq!(depth, DEFAULT_PROJECT_SYMLINK_MAX_DEPTH);

        let version: i64 = conn
            .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
            .expect("read schema version");
        assert_eq!(version, 5);
    }

    #[test]
    fn preserves_user_customized_ignored_dirs_during_v5_migration() {
        let conn = Connection::open_in_memory().expect("open in-memory connection");
        let custom = ".git\nnode_modules\nmy-custom-dir";
        seed_minimal_v4_settings(&conn, custom);
        migrate_to_v5(&conn).expect("migrate to v5");

        let value: String = conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'sync_project_symlink_ignored_dirs'",
                [],
                |row| row.get(0),
            )
            .expect("read ignored dirs");
        assert_eq!(value, custom);
    }
}
