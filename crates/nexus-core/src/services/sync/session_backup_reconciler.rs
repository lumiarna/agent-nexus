use rusqlite::params;

use crate::{database::Database, error::AppResult, services::util::now_epoch_seconds};

use super::{
    render_project_template, session_backup_source, SESSION_BACKUP_GROUP_ID,
    SESSION_BACKUP_SCHEDULE, SESSION_BACKUP_SYSTEM_KIND, SESSION_BACKUP_TARGET_TEMPLATE,
};

pub(super) struct SessionBackupReconciler<'a> {
    db: &'a Database,
}

impl<'a> SessionBackupReconciler<'a> {
    pub(super) fn new(db: &'a Database) -> Self {
        Self { db }
    }

    pub(super) fn reconcile(&self) -> AppResult<()> {
        let now = now_epoch_seconds()?;
        let mut conn = self.db.connection()?;
        let tx = conn.transaction()?;
        tx.execute(
            r#"
            INSERT INTO task_groups (id, name, system_kind, created_at, updated_at)
            VALUES (?1, 'Session Backup', ?2, ?3, ?3)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                system_kind = excluded.system_kind,
                updated_at = excluded.updated_at
            "#,
            params![SESSION_BACKUP_GROUP_ID, SESSION_BACKUP_SYSTEM_KIND, now],
        )?;

        let projects = {
            let mut stmt = tx.prepare("SELECT id, path, key, sessions_dir FROM projects")?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })?;
            rows.collect::<Result<Vec<_>, _>>()?
        };

        for (project_id, project_dir, project_key, sessions_dir) in projects {
            let project_dir = project_dir.trim_end_matches('/');
            let source = session_backup_source(project_dir, &sessions_dir);
            let target =
                render_project_template(SESSION_BACKUP_TARGET_TEMPLATE, project_dir, &project_key)?;
            tx.execute(
                r#"
                INSERT INTO tasks (
                    id, group_id, direction, action, source_type, source, target_type, target,
                    schedule, sort_index, last_status, project_id, created_at, updated_at
                )
                VALUES (?1, ?2, 'Push', 'Copy', 'Local', ?3, 'Cloud', ?4,
                        ?5, 0, 'never', ?6, ?7, ?7)
                ON CONFLICT(id) DO UPDATE SET
                    group_id = excluded.group_id,
                    direction = excluded.direction,
                    action = excluded.action,
                    source_type = excluded.source_type,
                    source = excluded.source,
                    target_type = excluded.target_type,
                    target = excluded.target,
                    project_id = excluded.project_id,
                    updated_at = excluded.updated_at
                "#,
                params![
                    format!("session-backup:{project_id}"),
                    SESSION_BACKUP_GROUP_ID,
                    source,
                    target,
                    SESSION_BACKUP_SCHEDULE,
                    project_id,
                    now,
                ],
            )?;
        }

        tx.execute(
            r#"
            DELETE FROM tasks
            WHERE group_id = ?1
              AND (project_id IS NULL OR NOT EXISTS (
                  SELECT 1 FROM projects WHERE projects.id = tasks.project_id
              ))
            "#,
            [SESSION_BACKUP_GROUP_ID],
        )?;
        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;

    #[test]
    fn materializes_session_backup_tasks_for_projects() {
        let db = Database::open_in_memory().expect("open in-memory database");
        {
            let conn = db.connection().expect("open connection");
            insert_project(&conn, "p1", "/workspace/agent-nexus/", "agent-nexus");
        }

        SessionBackupReconciler::new(&db)
            .reconcile()
            .expect("reconcile backups");

        let conn = db.connection().expect("open connection");
        let (source, target, schedule): (String, String, String) = conn
            .query_row(
                "SELECT source, target, schedule FROM tasks WHERE id = 'session-backup:p1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("load task");
        assert_eq!(source, "/workspace/agent-nexus/.sessions/");
        assert_eq!(target, "Session/agent-nexus/");
        assert_eq!(schedule, SESSION_BACKUP_SCHEDULE);
    }

    #[test]
    fn materializes_session_backup_source_from_custom_session_dir() {
        let db = Database::open_in_memory().expect("open in-memory database");
        {
            let conn = db.connection().expect("open connection");
            conn.execute(
                "INSERT INTO projects (id, name, path, key, sessions_dir, created_at, updated_at)
                 VALUES ('p1', 'agent-nexus', '/workspace/agent-nexus/', 'agent-nexus', 'notes', 0, 0)",
                [],
            )
            .expect("insert project");
        }

        SessionBackupReconciler::new(&db)
            .reconcile()
            .expect("reconcile backups");

        let conn = db.connection().expect("open connection");
        let source: String = conn
            .query_row(
                "SELECT source FROM tasks WHERE id = 'session-backup:p1'",
                [],
                |row| row.get(0),
            )
            .expect("load task");
        assert_eq!(source, "/workspace/agent-nexus/notes/");
    }

    #[test]
    fn removes_orphaned_system_backup_tasks() {
        let db = Database::open_in_memory().expect("open in-memory database");
        {
            let conn = db.connection().expect("open connection");
            conn.execute(
                "INSERT INTO task_groups (id, name, system_kind, created_at, updated_at)
                 VALUES (?1, 'Session Backup', ?2, 0, 0)",
                params![SESSION_BACKUP_GROUP_ID, SESSION_BACKUP_SYSTEM_KIND],
            )
            .expect("insert system group");
            conn.execute(
                "INSERT INTO tasks (
                    id, group_id, direction, action, source_type, source, target_type, target,
                    schedule, last_status, created_at, updated_at
                 )
                 VALUES (
                    'session-backup:missing', ?1, 'Push', 'Copy', 'Local', '/missing/.sessions/',
                    'Cloud', 'Session/missing/', ?2, 'never', 0, 0
                 )",
                params![SESSION_BACKUP_GROUP_ID, SESSION_BACKUP_SCHEDULE],
            )
            .expect("insert orphan task");
        }

        SessionBackupReconciler::new(&db)
            .reconcile()
            .expect("reconcile backups");

        let conn = db.connection().expect("open connection");
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM tasks", [], |row| row.get(0))
            .expect("count tasks");
        assert_eq!(count, 0);
    }

    fn insert_project(conn: &rusqlite::Connection, id: &str, path: &str, key: &str) {
        conn.execute(
            "INSERT INTO projects (id, name, path, key, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 0, 0)",
            params![id, key, path, key],
        )
        .expect("insert project");
    }
}
