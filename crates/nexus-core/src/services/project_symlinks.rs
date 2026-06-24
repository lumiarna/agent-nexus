use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use rusqlite::{OptionalExtension, Row};
use serde::Serialize;

use crate::{
    database::Database,
    error::{AppError, AppResult},
    services::{
        distribution, paths,
        placement::{
            remove_unmanaged_link_placement, scanned_target_identity,
            task_managed_target_identities,
        },
    },
};

const PROJECT_SYMLINK_IGNORED_DIRS_SETTING: &str = "project_symlink_ignored_dirs";
const PROJECT_SYMLINK_MAX_DEPTH_SETTING: &str = "project_symlink_max_depth";
const DEFAULT_PROJECT_SYMLINK_MAX_DEPTH: usize = 3;
const DEFAULT_PROJECT_SYMLINK_IGNORED_DIRS: &[&str] = &[
    ".git",
    ".venv",
    "node_modules",
    "target",
    "dist",
    "build",
    "out",
    "__pycache__",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
    ".next",
    ".nuxt",
    ".turbo",
    ".svelte-kit",
    ".gradle",
    ".idea",
    "coverage",
    ".tox",
    ".cache",
];

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
    pub link_type: String,
    pub link_kind: String,
    pub status: String,
}

#[derive(Clone)]
pub struct ProjectSymlinkInventory {
    db: Arc<Database>,
}

#[derive(Debug, Clone)]
struct ProjectRoot {
    id: String,
    name: String,
    path: PathBuf,
}

struct ScanContext<'a> {
    target_project: &'a ProjectRoot,
    projects: &'a [ProjectRoot],
    ignored_dirs: &'a HashSet<String>,
    max_depth: usize,
    seen_targets: &'a mut HashSet<String>,
    links: &'a mut Vec<ProjectSymlink>,
}

impl ProjectSymlinkInventory {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    pub fn list_project_symlinks(&self) -> AppResult<Vec<ProjectSymlink>> {
        let projects = self.list_existing_project_roots()?;
        let ignored_dirs = self.project_symlink_ignored_dirs()?;
        let max_depth = self.project_symlink_max_depth()?;
        let managed_targets = {
            let conn = self.db.connection()?;
            let mut targets = task_managed_target_identities(&conn)?;
            targets.extend(distribution::project_managed_target_identities(&conn)?);
            targets
        };
        let mut links = Vec::new();
        let mut seen_targets = HashSet::new();

        for project in &projects {
            collect_project_symlinks(
                project,
                &projects,
                &ignored_dirs,
                max_depth,
                &mut seen_targets,
                &mut links,
            )?;
        }

        let mut unmanaged_links = Vec::new();
        for link in links {
            if !managed_targets.contains(&scanned_target_identity(Path::new(&link.target_path))?) {
                unmanaged_links.push(link);
            }
        }
        let mut links = unmanaged_links;

        let project_order = projects
            .iter()
            .enumerate()
            .map(|(index, project)| (project.id.as_str(), index))
            .collect::<HashMap<_, _>>();

        links.sort_by(|left, right| {
            project_display_index(left, &project_order)
                .cmp(&project_display_index(right, &project_order))
                .then_with(|| {
                    project_index(left.target_project_id.as_deref(), &project_order).cmp(
                        &project_index(right.target_project_id.as_deref(), &project_order),
                    )
                })
                .then_with(|| left.source_path.cmp(&right.source_path))
                .then_with(|| left.target_path.cmp(&right.target_path))
        });
        Ok(links)
    }

    pub fn delete_project_symlink(&self, target_path: String) -> AppResult<()> {
        let target_path = required_trimmed(&target_path, "project symlink target path")?;
        remove_unmanaged_link_placement(Path::new(target_path))
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

    fn project_symlink_max_depth(&self) -> AppResult<usize> {
        let conn = self.db.connection()?;
        let value = conn
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                [PROJECT_SYMLINK_MAX_DEPTH_SETTING],
                |row| row.get::<_, String>(0),
            )
            .optional()?;

        Ok(match value {
            Some(raw) => raw
                .trim()
                .parse::<usize>()
                .unwrap_or(DEFAULT_PROJECT_SYMLINK_MAX_DEPTH),
            None => DEFAULT_PROJECT_SYMLINK_MAX_DEPTH,
        })
    }
}

fn required_trimmed<'a>(value: &'a str, label: &str) -> AppResult<&'a str> {
    let value = value.trim();
    if value.is_empty() {
        Err(AppError::Validation(format!("{label} is required")))
    } else {
        Ok(value)
    }
}

fn collect_project_symlinks(
    project: &ProjectRoot,
    projects: &[ProjectRoot],
    ignored_dirs: &HashSet<String>,
    max_depth: usize,
    seen_targets: &mut HashSet<String>,
    links: &mut Vec<ProjectSymlink>,
) -> AppResult<()> {
    let mut ctx = ScanContext {
        target_project: project,
        projects,
        ignored_dirs,
        max_depth,
        seen_targets,
        links,
    };
    collect_symlinks_in_dir(&mut ctx, &project.path, 0)
}

fn collect_symlinks_in_dir(ctx: &mut ScanContext<'_>, dir: &Path, depth: usize) -> AppResult<()> {
    if depth >= ctx.max_depth {
        return Ok(());
    }

    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return Ok(()),
    };

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let path = entry.path();
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();

        if ctx.ignored_dirs.contains(file_name.as_ref()) {
            continue;
        }

        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };

        // Check junction before symlink: on Windows a junction can also report
        // is_symlink(), so the more specific reparse kind must win.
        #[cfg(windows)]
        if junction::exists(&path).unwrap_or(false) {
            if let Ok(raw_source) = junction::get_target(&path) {
                push_project_symlink(ctx, &path, "Junction", raw_source);
            }
            continue;
        }

        if metadata.file_type().is_symlink() {
            if let Ok(raw_source) = fs::read_link(&path) {
                push_project_symlink(ctx, &path, "Symlink", raw_source);
            }
            continue;
        }

        if metadata.is_dir() {
            collect_symlinks_in_dir(ctx, &path, depth + 1)?;
        }
    }

    Ok(())
}

fn push_project_symlink(
    ctx: &mut ScanContext<'_>,
    path: &Path,
    link_type: &str,
    raw_source: PathBuf,
) {
    if let Some(link) = project_symlink_for_entry(
        path,
        link_type,
        raw_source,
        ctx.target_project,
        ctx.projects,
    ) {
        if ctx.seen_targets.insert(link.target_path.clone()) {
            ctx.links.push(link);
        }
    }
}

fn project_symlink_for_entry(
    path: &Path,
    link_type: &str,
    raw_source: PathBuf,
    target_project: &ProjectRoot,
    projects: &[ProjectRoot],
) -> Option<ProjectSymlink> {
    let target_path = path_to_string(path).ok()?;
    project_symlink_from_path(
        path,
        link_type,
        raw_source,
        target_project,
        projects,
        target_path,
    )
    .ok()
}

fn parse_ignored_dirs(value: &str) -> HashSet<String> {
    value
        .lines()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn project_display_index(link: &ProjectSymlink, project_order: &HashMap<&str, usize>) -> usize {
    link.source_project_id
        .as_deref()
        .or(link.target_project_id.as_deref())
        .map(|id| project_index(Some(id), project_order))
        .unwrap_or(usize::MAX)
}

fn project_index(project_id: Option<&str>, project_order: &HashMap<&str, usize>) -> usize {
    project_id
        .and_then(|id| project_order.get(id).copied())
        .unwrap_or(usize::MAX)
}

fn project_symlink_from_path(
    path: &Path,
    link_type: &str,
    raw_source: PathBuf,
    target_project: &ProjectRoot,
    projects: &[ProjectRoot],
    target_path: String,
) -> AppResult<ProjectSymlink> {
    let resolved_source = if raw_source.is_absolute() {
        raw_source
    } else {
        path.parent()
            .map(|parent| parent.join(&raw_source))
            .unwrap_or(raw_source)
    };
    let source_exists = resolved_source.exists();
    let canonical_source = if source_exists {
        Some(resolved_source.canonicalize()?)
    } else {
        None
    };
    let source_for_display = match &canonical_source {
        Some(path) => paths::path_buf_for_comparison(path.clone(), "project symlink source path")?,
        None => resolved_source.clone(),
    };
    let source_project = projects
        .iter()
        .find(|project| source_for_display.starts_with(&project.path));
    let source_for_metadata = canonical_source.as_deref().unwrap_or(&resolved_source);

    Ok(ProjectSymlink {
        id: target_path.clone(),
        source_path: path_to_string(&source_for_display)?,
        source_project_id: source_project.map(|project| project.id.clone()),
        source_project_name: source_project.map(|project| project.name.clone()),
        target_path,
        target_project_id: Some(target_project.id.clone()),
        target_project_name: Some(target_project.name.clone()),
        link_type: link_type.to_string(),
        link_kind: link_kind(source_for_metadata),
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
    paths::path_to_string(path, "path")
}

fn project_root_from_row(row: &Row<'_>) -> rusqlite::Result<ProjectRoot> {
    let path: String = row.get(2)?;
    Ok(ProjectRoot {
        id: row.get(0)?,
        name: row.get(1)?,
        path: PathBuf::from(path),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_symlinks_in_dir_returns_ok_when_dir_unreadable() {
        let missing = std::env::temp_dir().join("agent-nexus-definitely-missing-xyz-test");
        let project = ProjectRoot {
            id: "p".to_string(),
            name: "p".to_string(),
            path: missing.clone(),
        };
        let ignored = HashSet::new();
        let mut seen = HashSet::new();
        let mut links = Vec::new();
        let mut ctx = ScanContext {
            target_project: &project,
            projects: &[],
            ignored_dirs: &ignored,
            max_depth: 3,
            seen_targets: &mut seen,
            links: &mut links,
        };

        collect_symlinks_in_dir(&mut ctx, &missing, 0)
            .expect("unreadable dir should not abort scan");

        assert!(links.is_empty());
    }
}
