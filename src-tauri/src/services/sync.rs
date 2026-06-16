use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use rusqlite::{OptionalExtension, Row};
use serde::Serialize;

use crate::{
    database::Database,
    error::{AppError, AppResult},
};

const PROJECT_SYMLINK_IGNORED_DIRS_SETTING: &str = "sync_project_symlink_ignored_dirs";
const DEFAULT_PROJECT_SYMLINK_IGNORED_DIRS: &[&str] = &[".git", ".venv", "node_modules"];

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSymlink {
    pub id: String,
    pub source_path: String,
    pub source_project_id: Option<String>,
    pub source_project_name: Option<String>,
    pub target_path: String,
    pub target_project_id: Option<String>,
    pub target_project_name: Option<String>,
    pub link_kind: String,
    pub status: String,
}

#[derive(Clone)]
pub struct SyncService {
    db: Arc<Database>,
}

#[derive(Debug, Clone)]
struct ProjectRoot {
    id: String,
    name: String,
    path: PathBuf,
}

impl SyncService {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    pub fn list_project_symlinks(&self) -> AppResult<Vec<ProjectSymlink>> {
        let projects = self.list_existing_project_roots()?;
        let ignored_dirs = self.project_symlink_ignored_dirs()?;
        let mut links = Vec::new();
        let mut seen_targets = HashSet::new();

        for project in &projects {
            collect_project_symlinks(
                project,
                &projects,
                &ignored_dirs,
                &mut seen_targets,
                &mut links,
            )?;
        }

        links.sort_by(|left, right| {
            left.target_path
                .cmp(&right.target_path)
                .then_with(|| left.source_path.cmp(&right.source_path))
        });
        Ok(links)
    }

    fn list_existing_project_roots(&self) -> AppResult<Vec<ProjectRoot>> {
        let conn = self.db.connection()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT id, name, path
            FROM projects
            ORDER BY sort_index IS NULL, sort_index, created_at, name
            "#,
        )?;

        let rows = stmt.query_map([], project_root_from_row)?;
        let mut projects = Vec::new();

        for row in rows {
            let project = row?;
            if project.path.is_dir() {
                projects.push(project);
            }
        }

        Ok(projects)
    }

    fn project_symlink_ignored_dirs(&self) -> AppResult<HashSet<String>> {
        let conn = self.db.connection()?;
        let value = conn
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                [PROJECT_SYMLINK_IGNORED_DIRS_SETTING],
                |row| row.get::<_, String>(0),
            )
            .optional()?;

        Ok(match value {
            Some(value) => parse_ignored_dirs(&value),
            None => DEFAULT_PROJECT_SYMLINK_IGNORED_DIRS
                .iter()
                .map(|value| value.to_string())
                .collect(),
        })
    }
}

fn collect_project_symlinks(
    project: &ProjectRoot,
    projects: &[ProjectRoot],
    ignored_dirs: &HashSet<String>,
    seen_targets: &mut HashSet<String>,
    links: &mut Vec<ProjectSymlink>,
) -> AppResult<()> {
    collect_symlinks_in_dir(
        &project.path,
        project,
        projects,
        ignored_dirs,
        seen_targets,
        links,
    )
}

fn collect_symlinks_in_dir(
    dir: &Path,
    target_project: &ProjectRoot,
    projects: &[ProjectRoot],
    ignored_dirs: &HashSet<String>,
    seen_targets: &mut HashSet<String>,
    links: &mut Vec<ProjectSymlink>,
) -> AppResult<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();

        if ignored_dirs.contains(file_name.as_ref()) {
            continue;
        }

        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            let target_path = path_to_string(&path)?;
            if seen_targets.insert(target_path.clone()) {
                links.push(project_symlink_from_path(
                    &path,
                    target_project,
                    projects,
                    target_path,
                )?);
            }
            continue;
        }

        if metadata.is_dir() {
            collect_symlinks_in_dir(
                &path,
                target_project,
                projects,
                ignored_dirs,
                seen_targets,
                links,
            )?;
        }
    }

    Ok(())
}

fn parse_ignored_dirs(value: &str) -> HashSet<String> {
    value
        .lines()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn project_symlink_from_path(
    path: &Path,
    target_project: &ProjectRoot,
    projects: &[ProjectRoot],
    target_path: String,
) -> AppResult<ProjectSymlink> {
    let raw_source = fs::read_link(path)?;
    let resolved_source = if raw_source.is_absolute() {
        raw_source
    } else {
        path.parent()
            .map(|parent| parent.join(&raw_source))
            .unwrap_or(raw_source)
    };
    let source_exists = resolved_source.exists();
    let comparable_source = if source_exists {
        resolved_source.canonicalize()?
    } else {
        resolved_source.clone()
    };
    let source_project = projects
        .iter()
        .find(|project| comparable_source.starts_with(&project.path));

    Ok(ProjectSymlink {
        id: target_path.clone(),
        source_path: path_to_string(&comparable_source)?,
        source_project_id: source_project.map(|project| project.id.clone()),
        source_project_name: source_project.map(|project| project.name.clone()),
        target_path,
        target_project_id: Some(target_project.id.clone()),
        target_project_name: Some(target_project.name.clone()),
        link_kind: link_kind(&comparable_source),
        status: if source_exists { "ok" } else { "missing" }.to_string(),
    })
}

fn link_kind(path: &Path) -> String {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => "directory".to_string(),
        Ok(metadata) if metadata.is_file() => "file".to_string(),
        Ok(_) => "other".to_string(),
        Err(_) => "missing".to_string(),
    }
}

fn path_to_string(path: &Path) -> AppResult<String> {
    path.to_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| AppError::Validation("path must be valid UTF-8".to_string()))
}

fn project_root_from_row(row: &Row<'_>) -> rusqlite::Result<ProjectRoot> {
    let path: String = row.get(2)?;
    Ok(ProjectRoot {
        id: row.get(0)?,
        name: row.get(1)?,
        path: PathBuf::from(path),
    })
}
