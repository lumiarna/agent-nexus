use rusqlite::{params, Connection};

use crate::error::{AppError, AppResult};

const CURRENT_SCHEMA_VERSION: i64 = 8;

const LEGACY_DEFAULT_PROJECT_SYMLINK_IGNORED_DIRS: &str = ".git\n.venv\nnode_modules";
const NEW_DEFAULT_PROJECT_SYMLINK_IGNORED_DIRS: &str = ".git\n.venv\nnode_modules\ntarget\ndist\nbuild\nout\n__pycache__\n.pytest_cache\n.mypy_cache\n.ruff_cache\n.next\n.nuxt\n.turbo\n.svelte-kit\n.gradle\n.idea\ncoverage\n.tox\n.cache";
const DEFAULT_PROJECT_SYMLINK_MAX_DEPTH: &str = "3";
const LEGACY_PROJECT_SYMLINK_IGNORED_DIRS_KEY: &str = "sync_project_symlink_ignored_dirs";
const LEGACY_PROJECT_SYMLINK_MAX_DEPTH_KEY: &str = "sync_project_symlink_max_depth";
const PROJECT_SYMLINK_IGNORED_DIRS_KEY: &str = "project_symlink_ignored_dirs";
const PROJECT_SYMLINK_MAX_DEPTH_KEY: &str = "project_symlink_max_depth";
const CLAUDE_CONFIG_DIR_KEY: &str = "CLAUDE_CONFIG_DIR";
const DEFAULT_CLAUDE_CONFIG_DIR: &str = "~/.claude";

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
        if current < 6 {
            migrate_to_v6(conn)?;
        }
        if current < 7 {
            migrate_to_v7(conn)?;
        }
        if current < 8 {
            migrate_to_v8(conn)?;
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
        INSERT INTO schema_version (version) VALUES (7);

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
            action TEXT NOT NULL CHECK (action IN ('Symlink', 'Junction', 'Copy')),
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
        VALUES ('project_symlink_ignored_dirs', '{new_default_ignored_dirs}');
        INSERT INTO settings (key, value)
        VALUES ('project_symlink_max_depth', '{default_max_depth}');
        INSERT INTO settings (key, value)
        VALUES ('{claude_config_dir_key}', '{default_claude_config_dir}');

        COMMIT;
        "#,
        new_default_ignored_dirs = NEW_DEFAULT_PROJECT_SYMLINK_IGNORED_DIRS,
        default_max_depth = DEFAULT_PROJECT_SYMLINK_MAX_DEPTH,
        claude_config_dir_key = CLAUDE_CONFIG_DIR_KEY,
        default_claude_config_dir = DEFAULT_CLAUDE_CONFIG_DIR,
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
            action TEXT NOT NULL CHECK (action IN ('Symlink', 'Junction', 'Copy')),
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

fn migrate_to_v6(conn: &Connection) -> AppResult<()> {
    // Promote Junction to a first-class task action. SQLite cannot alter a CHECK
    // constraint in place, so rebuild `tasks` with the widened constraint while
    // preserving existing rows.
    conn.execute_batch(
        r#"
        BEGIN;
        CREATE TABLE tasks_v6 (
            id TEXT PRIMARY KEY,
            group_id TEXT NOT NULL,
            direction TEXT NOT NULL
                CHECK (direction IN ('Distribution', 'Push', 'Pull')),
            action TEXT NOT NULL CHECK (action IN ('Symlink', 'Junction', 'Copy')),
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
        INSERT INTO tasks_v6 SELECT * FROM tasks;
        DROP TABLE tasks;
        ALTER TABLE tasks_v6 RENAME TO tasks;
        CREATE INDEX idx_tasks_group ON tasks(group_id);
        UPDATE schema_version SET version = 6;
        COMMIT;
        "#,
    )
    .or_else(|error| {
        let _ = conn.execute("ROLLBACK", params![]);
        Err(error)
    })?;

    Ok(())
}

fn migrate_to_v7(conn: &Connection) -> AppResult<()> {
    conn.execute_batch("BEGIN;").or_else(|error| {
        let _ = conn.execute("ROLLBACK", params![]);
        Err(error)
    })?;

    let result = (|| -> AppResult<()> {
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO NOTHING",
            params![
                PROJECT_SYMLINK_IGNORED_DIRS_KEY,
                NEW_DEFAULT_PROJECT_SYMLINK_IGNORED_DIRS
            ],
        )?;
        conn.execute(
            "INSERT INTO settings (key, value) \
             SELECT ?1, value FROM settings WHERE key = ?2 \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![
                PROJECT_SYMLINK_IGNORED_DIRS_KEY,
                LEGACY_PROJECT_SYMLINK_IGNORED_DIRS_KEY
            ],
        )?;
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO NOTHING",
            params![
                PROJECT_SYMLINK_MAX_DEPTH_KEY,
                DEFAULT_PROJECT_SYMLINK_MAX_DEPTH
            ],
        )?;
        conn.execute(
            "INSERT INTO settings (key, value) \
             SELECT ?1, value FROM settings WHERE key = ?2 \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![
                PROJECT_SYMLINK_MAX_DEPTH_KEY,
                LEGACY_PROJECT_SYMLINK_MAX_DEPTH_KEY
            ],
        )?;
        conn.execute(
            "DELETE FROM settings WHERE key IN (?1, ?2)",
            params![
                LEGACY_PROJECT_SYMLINK_IGNORED_DIRS_KEY,
                LEGACY_PROJECT_SYMLINK_MAX_DEPTH_KEY
            ],
        )?;
        conn.execute("UPDATE schema_version SET version = 7", [])?;
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

fn migrate_to_v8(conn: &Connection) -> AppResult<()> {
    conn.execute_batch("BEGIN;").or_else(|error| {
        let _ = conn.execute("ROLLBACK", params![]);
        Err(error)
    })?;

    let result = (|| -> AppResult<()> {
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO NOTHING",
            params![CLAUDE_CONFIG_DIR_KEY, DEFAULT_CLAUDE_CONFIG_DIR],
        )?;
        conn.execute("UPDATE schema_version SET version = 8", [])?;
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
        conn.execute("CREATE TABLE schema_version (version INTEGER NOT NULL)", [])
            .expect("create schema_version table");
        conn.execute("INSERT INTO schema_version (version) VALUES (4)", [])
            .expect("seed schema_version 4");
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('sync_project_symlink_ignored_dirs', ?1)",
            [ignored_dirs_value],
        )
        .expect("seed ignored dirs");
    }

    fn seed_minimal_v6_project_symlink_settings(
        conn: &Connection,
        ignored_dirs_value: &str,
        max_depth_value: &str,
    ) {
        conn.execute(
            "CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
            [],
        )
        .expect("create settings table");
        conn.execute("CREATE TABLE schema_version (version INTEGER NOT NULL)", [])
            .expect("create schema_version table");
        conn.execute("INSERT INTO schema_version (version) VALUES (6)", [])
            .expect("seed schema_version 6");
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('sync_project_symlink_ignored_dirs', ?1)",
            [ignored_dirs_value],
        )
        .expect("seed legacy ignored dirs");
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('sync_project_symlink_max_depth', ?1)",
            [max_depth_value],
        )
        .expect("seed legacy max depth");
    }

    fn setting_value(conn: &Connection, key: &str) -> String {
        conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
            row.get(0)
        })
        .expect("read setting")
    }

    fn setting_count(conn: &Connection, key: &str) -> i64 {
        conn.query_row(
            "SELECT COUNT(*) FROM settings WHERE key = ?1",
            [key],
            |row| row.get(0),
        )
        .expect("count setting")
    }

    #[test]
    fn new_databases_seed_project_symlink_settings_with_project_keys() {
        let db = crate::database::Database::open_in_memory().expect("open in-memory database");
        let conn = db.connection().expect("open db connection");

        assert_eq!(
            setting_value(&conn, "project_symlink_ignored_dirs"),
            NEW_DEFAULT_PROJECT_SYMLINK_IGNORED_DIRS
        );
        assert_eq!(
            setting_value(&conn, "project_symlink_max_depth"),
            DEFAULT_PROJECT_SYMLINK_MAX_DEPTH
        );
        assert_eq!(
            setting_value(&conn, CLAUDE_CONFIG_DIR_KEY),
            DEFAULT_CLAUDE_CONFIG_DIR
        );
        assert_eq!(setting_count(&conn, "sync_project_symlink_ignored_dirs"), 0);
        assert_eq!(setting_count(&conn, "sync_project_symlink_max_depth"), 0);
    }

    #[test]
    fn migrates_v7_databases_with_default_claude_config_dir() {
        let conn = Connection::open_in_memory().expect("open in-memory connection");
        seed_minimal_v6_project_symlink_settings(
            &conn,
            NEW_DEFAULT_PROJECT_SYMLINK_IGNORED_DIRS,
            DEFAULT_PROJECT_SYMLINK_MAX_DEPTH,
        );
        migrate_to_v7(&conn).expect("migrate to v7");
        migrate(&conn).expect("migrate schema");

        assert_eq!(
            setting_value(&conn, CLAUDE_CONFIG_DIR_KEY),
            DEFAULT_CLAUDE_CONFIG_DIR
        );
        let version: i64 = conn
            .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
            .expect("read schema version");
        assert_eq!(version, CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn migrates_legacy_default_project_symlink_settings_to_project_keys() {
        let conn = Connection::open_in_memory().expect("open in-memory connection");
        seed_minimal_v6_project_symlink_settings(
            &conn,
            NEW_DEFAULT_PROJECT_SYMLINK_IGNORED_DIRS,
            DEFAULT_PROJECT_SYMLINK_MAX_DEPTH,
        );
        migrate(&conn).expect("migrate schema");

        assert_eq!(
            setting_value(&conn, "project_symlink_ignored_dirs"),
            NEW_DEFAULT_PROJECT_SYMLINK_IGNORED_DIRS
        );
        assert_eq!(
            setting_value(&conn, "project_symlink_max_depth"),
            DEFAULT_PROJECT_SYMLINK_MAX_DEPTH
        );
        assert_eq!(setting_count(&conn, "sync_project_symlink_ignored_dirs"), 0);
        assert_eq!(setting_count(&conn, "sync_project_symlink_max_depth"), 0);
        let version: i64 = conn
            .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
            .expect("read schema version");
        assert_eq!(version, CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn migrates_user_customized_project_symlink_settings_to_project_keys() {
        let conn = Connection::open_in_memory().expect("open in-memory connection");
        let custom_ignored_dirs = ".git\nnode_modules\nvendor";
        let custom_max_depth = "5";
        seed_minimal_v6_project_symlink_settings(&conn, custom_ignored_dirs, custom_max_depth);
        migrate(&conn).expect("migrate schema");

        assert_eq!(
            setting_value(&conn, "project_symlink_ignored_dirs"),
            custom_ignored_dirs
        );
        assert_eq!(
            setting_value(&conn, "project_symlink_max_depth"),
            custom_max_depth
        );
        assert_eq!(setting_count(&conn, "sync_project_symlink_ignored_dirs"), 0);
        assert_eq!(setting_count(&conn, "sync_project_symlink_max_depth"), 0);
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

    #[test]
    fn migrate_to_v6_allows_junction_action_and_preserves_rows() {
        let conn = Connection::open_in_memory().expect("open in-memory connection");
        conn.execute_batch(
            r#"
            CREATE TABLE schema_version (version INTEGER NOT NULL);
            INSERT INTO schema_version (version) VALUES (5);
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
            INSERT INTO task_groups (id, name, created_at, updated_at) VALUES ('g1', 'G', 0, 0);
            INSERT INTO tasks (id, group_id, direction, action, source_type, source, target_type, target, schedule, created_at, updated_at)
            VALUES ('t1', 'g1', 'Distribution', 'Symlink', 'Local', '/s', 'Local', '/t', 'manual', 0, 0);
            "#,
        )
        .expect("seed v5 tasks");

        migrate_to_v6(&conn).expect("migrate to v6");

        let version: i64 = conn
            .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
            .expect("read schema version");
        assert_eq!(version, 6);

        let kept: String = conn
            .query_row("SELECT action FROM tasks WHERE id = 't1'", [], |row| {
                row.get(0)
            })
            .expect("existing row preserved");
        assert_eq!(kept, "Symlink");

        conn.execute(
            "INSERT INTO tasks (id, group_id, direction, action, source_type, source, target_type, target, schedule, created_at, updated_at) \
             VALUES ('t2', 'g1', 'Distribution', 'Junction', 'Local', '/s', 'Local', '/t2', 'manual', 0, 0)",
            [],
        )
        .expect("rebuilt tasks table accepts Junction action");
    }
}
