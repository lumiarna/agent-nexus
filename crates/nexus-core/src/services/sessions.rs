use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
    time::UNIX_EPOCH,
};

use rusqlite::{params, Row};
use serde::Serialize;

use crate::{
    database::Database,
    error::{AppError, AppResult},
    services::paths,
};

const MAX_SESSION_INDEX_BYTES: u64 = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    pub title: String,
    pub project: String,
    pub project_name: String,
    pub file: String,
    pub size: String,
    pub updated: String,
    pub source: String,
    pub excerpt: String,
    pub body: String,
}

#[derive(Clone)]
pub struct SessionService {
    db: Arc<Database>,
}

#[derive(Debug, Clone)]
struct ProjectSessionRoot {
    id: String,
    path: PathBuf,
    sessions_dir: String,
}

#[derive(Debug, Clone)]
struct DiscoveredLocalSession {
    id: String,
    title: String,
    project_id: String,
    file_path: String,
    size_bytes: i64,
    updated_at: i64,
    excerpt: String,
}

#[derive(Debug, Clone)]
struct IndexedSession {
    id: String,
    title: String,
    project_id: String,
    project_name: String,
    project_path: PathBuf,
    sessions_dir: String,
    file_path: String,
    display_file: String,
    size_bytes: i64,
    updated: String,
    source: String,
    excerpt: String,
}

#[derive(Debug, Clone)]
struct ParsedMarkdown {
    title: String,
    excerpt: String,
    body: String,
}

impl SessionService {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    pub fn list_local_sessions(&self) -> AppResult<Vec<Session>> {
        let conn = self.db.connection()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT
                s.id,
                s.title,
                s.project_id,
                p.name,
                p.path,
                p.sessions_dir,
                s.file_path,
                CASE
                    WHEN p.sessions_dir = '' THEN s.file_path
                    ELSE p.sessions_dir || '/' || s.file_path
                END AS display_file,
                COALESCE(s.size_bytes, 0),
                COALESCE(strftime('%Y-%m-%d %H:%M', s.updated_at, 'unixepoch', 'localtime'), ''),
                s.source,
                COALESCE(s.excerpt, '')
            FROM session_index s
            JOIN projects p ON p.id = s.project_id
            WHERE s.source = 'local'
            ORDER BY s.updated_at DESC, s.title, s.file_path
            "#,
        )?;
        let rows = stmt.query_map([], indexed_session_from_row)?;
        let indexed = rows.collect::<Result<Vec<_>, _>>()?;
        drop(stmt);
        drop(conn);

        indexed
            .into_iter()
            .map(session_metadata_from_indexed)
            .collect::<AppResult<Vec<_>>>()
    }

    pub fn get_local_session(&self, id: String) -> AppResult<Session> {
        let id = id.trim();
        if id.is_empty() {
            return Err(AppError::Validation("session id is required".to_string()));
        }

        let conn = self.db.connection()?;
        let indexed = conn.query_row(
            r#"
            SELECT
                s.id,
                s.title,
                s.project_id,
                p.name,
                p.path,
                p.sessions_dir,
                s.file_path,
                CASE
                    WHEN p.sessions_dir = '' THEN s.file_path
                    ELSE p.sessions_dir || '/' || s.file_path
                END AS display_file,
                COALESCE(s.size_bytes, 0),
                COALESCE(strftime('%Y-%m-%d %H:%M', s.updated_at, 'unixepoch', 'localtime'), ''),
                s.source,
                COALESCE(s.excerpt, '')
            FROM session_index s
            JOIN projects p ON p.id = s.project_id
            WHERE s.id = ?1 AND s.source = 'local'
            "#,
            params![id],
            indexed_session_from_row,
        )?;
        drop(conn);

        session_with_body_from_indexed(indexed)
    }

    pub fn scan_local_sessions(&self) -> AppResult<Vec<Session>> {
        let roots = self.list_local_session_roots()?;
        let mut discovered = Vec::new();

        for root in roots {
            let session_root = resolve_session_root(&root.path, &root.sessions_dir);
            if !session_root.exists() {
                continue;
            }
            if !session_root.is_dir() {
                return Err(AppError::Validation(format!(
                    "session directory is not a directory: {}",
                    session_root.display()
                )));
            }

            let mut files = Vec::new();
            collect_markdown_files(&session_root, &mut files)?;
            for file in files {
                discovered.push(read_local_session_file(&root, &session_root, &file)?);
            }
        }

        discovered.sort_by(|left, right| {
            right
                .updated_at
                .cmp(&left.updated_at)
                .then_with(|| left.title.cmp(&right.title))
                .then_with(|| left.file_path.cmp(&right.file_path))
        });

        let mut conn = self.db.connection()?;
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM session_index WHERE source = 'local'", [])?;
        for session in &discovered {
            tx.execute(
                r#"
                INSERT INTO session_index (
                    id, project_id, title, file_path, excerpt, source, size_bytes, updated_at
                )
                VALUES (?1, ?2, ?3, ?4, ?5, 'local', ?6, ?7)
                "#,
                params![
                    session.id,
                    session.project_id,
                    session.title,
                    session.file_path,
                    session.excerpt,
                    session.size_bytes,
                    session.updated_at,
                ],
            )?;
        }
        tx.execute(
            "INSERT INTO session_fts(session_fts) VALUES ('rebuild')",
            [],
        )?;
        tx.commit()?;
        drop(conn);

        self.list_local_sessions()
    }

    fn list_local_session_roots(&self) -> AppResult<Vec<ProjectSessionRoot>> {
        let conn = self.db.connection()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT id, path, sessions_dir
            FROM projects
            WHERE status = 'active'
            ORDER BY sort_index IS NULL, sort_index, created_at, name
            "#,
        )?;
        let rows = stmt.query_map([], |row| {
            let path: String = row.get(1)?;
            Ok(ProjectSessionRoot {
                id: row.get(0)?,
                path: PathBuf::from(path),
                sessions_dir: row.get(2)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}

fn session_metadata_from_indexed(value: IndexedSession) -> AppResult<Session> {
    Ok(Session {
        id: value.id,
        title: value.title,
        project: value.project_id,
        project_name: value.project_name,
        file: value.display_file,
        size: format_size(value.size_bytes),
        updated: value.updated,
        source: value.source,
        excerpt: value.excerpt,
        body: String::new(),
    })
}

fn session_with_body_from_indexed(value: IndexedSession) -> AppResult<Session> {
    let full_path =
        resolve_session_root(&value.project_path, &value.sessions_dir).join(&value.file_path);
    let body = parse_session_markdown(
        &fs::read_to_string(&full_path)?,
        fallback_title_from_path(Path::new(&value.file_path))?,
    )
    .body;

    Ok(Session {
        id: value.id,
        title: value.title,
        project: value.project_id,
        project_name: value.project_name,
        file: value.display_file,
        size: format_size(value.size_bytes),
        updated: value.updated,
        source: value.source,
        excerpt: value.excerpt,
        body,
    })
}

fn indexed_session_from_row(row: &Row<'_>) -> rusqlite::Result<IndexedSession> {
    let project_path: String = row.get(4)?;
    Ok(IndexedSession {
        id: row.get(0)?,
        title: row.get(1)?,
        project_id: row.get(2)?,
        project_name: row.get(3)?,
        project_path: PathBuf::from(project_path),
        sessions_dir: row.get(5)?,
        file_path: row.get(6)?,
        display_file: row.get(7)?,
        size_bytes: row.get(8)?,
        updated: row.get(9)?,
        source: row.get(10)?,
        excerpt: row.get(11)?,
    })
}

fn read_local_session_file(
    project: &ProjectSessionRoot,
    session_root: &Path,
    file: &Path,
) -> AppResult<DiscoveredLocalSession> {
    let relative = file
        .strip_prefix(session_root)
        .map_err(|error| AppError::Internal(error.to_string()))?;
    let file_path = paths::path_to_string(relative, "session file path")?;
    let text = read_session_index_text(file)?;
    let parsed = parse_session_markdown(&text, fallback_title_from_path(relative)?);
    let metadata = fs::metadata(file)?;
    let modified = metadata.modified()?;
    let updated_at = modified
        .duration_since(UNIX_EPOCH)
        .map_err(|error| AppError::Internal(error.to_string()))?
        .as_secs() as i64;

    Ok(DiscoveredLocalSession {
        id: format!("local:{}:{}", project.id, file_path),
        title: parsed.title,
        project_id: project.id.clone(),
        file_path,
        size_bytes: metadata.len() as i64,
        updated_at,
        excerpt: parsed.excerpt,
    })
}

fn resolve_session_root(project_path: &Path, sessions_dir: &str) -> PathBuf {
    let sessions_dir = Path::new(sessions_dir);
    if sessions_dir.is_absolute() {
        sessions_dir.to_path_buf()
    } else {
        project_path.join(sessions_dir)
    }
}

fn collect_markdown_files(dir: &Path, files: &mut Vec<PathBuf>) -> AppResult<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let path = entry.path();
        if file_type.is_dir() {
            collect_markdown_files(&path, files)?;
        } else if file_type.is_file()
            && path.extension().and_then(|value| value.to_str()) == Some("md")
        {
            files.push(path);
        }
    }
    files.sort();
    Ok(())
}

fn read_session_index_text(path: &Path) -> AppResult<String> {
    let metadata = fs::metadata(path)?;
    let truncated = metadata.len() > MAX_SESSION_INDEX_BYTES;
    let mut file = fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.by_ref()
        .take(MAX_SESSION_INDEX_BYTES)
        .read_to_end(&mut bytes)?;
    valid_utf8_prefix(bytes, truncated, path)
}

fn valid_utf8_prefix(bytes: Vec<u8>, truncated: bool, path: &Path) -> AppResult<String> {
    match String::from_utf8(bytes) {
        Ok(text) => Ok(text),
        Err(error) => {
            let utf8_error = error.utf8_error();
            if truncated && utf8_error.error_len().is_none() {
                let valid_up_to = utf8_error.valid_up_to();
                let mut bytes = error.into_bytes();
                bytes.truncate(valid_up_to);
                String::from_utf8(bytes).map_err(|error| AppError::Validation(error.to_string()))
            } else {
                Err(AppError::Validation(format!(
                    "session file must be valid UTF-8: {}",
                    path.display()
                )))
            }
        }
    }
}

fn parse_session_markdown(text: &str, fallback_title: String) -> ParsedMarkdown {
    let normalized = text.replace("\r\n", "\n");
    let (metadata, body) = split_frontmatter(&normalized);
    let title = frontmatter_value(metadata, "name").unwrap_or(fallback_title);
    let body = body.trim_start().to_string();
    let excerpt = frontmatter_value(metadata, "description")
        .unwrap_or_else(|| first_body_line(&body).unwrap_or_default());

    ParsedMarkdown {
        title,
        excerpt,
        body,
    }
}

fn split_frontmatter(text: &str) -> (Option<&str>, &str) {
    let Some(rest) = text.strip_prefix("---\n") else {
        return (None, text);
    };
    let Some(end) = rest.find("\n---\n") else {
        return (None, text);
    };

    let metadata = &rest[..end];
    let body = &rest[end + "\n---\n".len()..];
    (Some(metadata), body)
}

fn frontmatter_value(metadata: Option<&str>, key: &str) -> Option<String> {
    metadata?
        .lines()
        .filter_map(|line| line.split_once(':'))
        .find_map(|(candidate, value)| {
            if candidate.trim() == key {
                let value = value.trim().trim_matches('"').trim_matches('\'');
                if value.is_empty() {
                    None
                } else {
                    Some(value.to_string())
                }
            } else {
                None
            }
        })
}

fn first_body_line(body: &str) -> Option<String> {
    body.lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#'))
        .map(ToOwned::to_owned)
}

fn fallback_title_from_path(path: &Path) -> AppResult<String> {
    path.file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| AppError::Validation("session file has no valid title".to_string()))
}

fn format_size(bytes: i64) -> String {
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let kib = bytes as f64 / 1024.0;
    if kib < 1024.0 {
        return format!("{kib:.1} KB");
    }
    format!("{:.1} MB", kib / 1024.0)
}
