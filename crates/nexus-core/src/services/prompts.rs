use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    database::Database,
    error::{AppError, AppResult},
    services::agent_capabilities::{
        agent_by_name as capability_by_name, agent_capability_surfaces, AgentCapabilitySurface,
    },
    services::paths,
    services::symlink::{create_symlink_placement, is_junction, remove_symlink_if_present},
};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Prompt {
    pub id: String,
    pub name: String,
    pub path: String,
    pub cells: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetPromptTargetInput {
    pub prompt_id: String,
    pub agent: String,
    pub enabled: bool,
}

#[derive(Clone)]
pub struct PromptService {
    db: Arc<Database>,
}

#[derive(Debug, Clone)]
struct PromptSource {
    source_agent: &'static str,
    name: String,
    canonical_path: PathBuf,
}

#[derive(Debug, Clone)]
struct PromptTargetContext {
    canonical_path: PathBuf,
    source_agent: String,
}

impl PromptService {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    pub fn list_prompts(&self) -> AppResult<Vec<Prompt>> {
        let conn = self.db.connection()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT id, name, canonical_path
            FROM prompts
            ORDER BY name, canonical_path
            "#,
        )?;
        let rows = stmt.query_map([], |row| prompt_from_row(row, &conn))?;
        let mut prompts = rows.collect::<Result<Vec<_>, _>>()?;
        prompts.sort_by_key(source_index);
        Ok(prompts)
    }

    pub fn scan_prompts(&self) -> AppResult<Vec<Prompt>> {
        let known_target_paths = self.known_prompt_target_paths()?;
        let mut sources = Vec::new();

        for agent in agent_capability_surfaces() {
            let Some(prompt) = agent.prompt else {
                continue;
            };
            let prompt_file = paths::resolve_local_path(prompt.global_file)?;
            if let Some(source) = discover_prompt_source(agent, &prompt_file, &known_target_paths)?
            {
                sources.push(source);
            }
        }

        sources.sort_by_key(|source| source_index_for_agent(source.source_agent));
        self.replace_scanned_sources(sources)?;
        self.list_prompts()
    }

    pub fn set_prompt_target(&self, input: SetPromptTargetInput) -> AppResult<Prompt> {
        let prompt_id = required_trimmed(&input.prompt_id, "prompt id")?;
        let target_agent = agent_by_name(required_trimmed(&input.agent, "agent")?)?;
        if target_agent.prompt.is_none() {
            return Err(AppError::Validation(
                "agent does not support prompt targets".to_string(),
            ));
        }

        let context = self.prompt_target_context(prompt_id)?;
        let source_agent = agent_by_name(&context.source_agent)?;
        if source_agent.name == target_agent.name {
            return Err(AppError::Validation(
                "source agent cannot be toggled as a target".to_string(),
            ));
        }

        let target_path = target_path_for_agent(target_agent)?.ok_or_else(|| {
            AppError::Validation("prompt target path cannot be computed".to_string())
        })?;

        let created_symlink = if input.enabled {
            create_symlink_placement(&context.canonical_path, &target_path)?;
            true
        } else {
            remove_symlink_if_present(&target_path)?;
            false
        };

        let result = (|| -> AppResult<Prompt> {
            let conn = self.db.connection()?;
            conn.execute(
                r#"
                INSERT INTO prompt_distributions (prompt_id, agent, role, target_path)
                VALUES (?1, ?2, ?3, ?4)
                ON CONFLICT(prompt_id, agent) DO UPDATE SET
                    role = excluded.role,
                    target_path = excluded.target_path
                "#,
                params![
                    prompt_id,
                    target_agent.name,
                    if input.enabled { "target" } else { "none" },
                    if input.enabled {
                        Some(path_to_string(&target_path, "prompt target path")?)
                    } else {
                        None
                    },
                ],
            )?;
            drop(conn);
            self.get_prompt(prompt_id)
        })();

        if result.is_err() && created_symlink {
            let _ = remove_symlink_if_present(&target_path);
        }

        result
    }

    pub fn open_prompt_source(&self, id: String) -> AppResult<()> {
        let id = required_trimmed(&id, "prompt id")?;
        let canonical_path = self.prompt_canonical_path(id)?;
        open_path(&canonical_path)
    }

    pub fn reveal_prompt_path(&self, id: String) -> AppResult<()> {
        let id = required_trimmed(&id, "prompt id")?;
        let canonical_path = self.prompt_canonical_path(id)?;
        reveal_path(&canonical_path)
    }

    fn replace_scanned_sources(&self, sources: Vec<PromptSource>) -> AppResult<()> {
        let now = now_epoch_seconds()?;
        let mut scanned_paths = BTreeSet::new();
        let mut conn = self.db.connection()?;
        let tx = conn.transaction()?;

        for source in sources {
            let canonical_path = path_to_string(&source.canonical_path, "prompt path")?;
            if !scanned_paths.insert(canonical_path.clone()) {
                continue;
            }

            let existing_id = tx
                .query_row(
                    "SELECT id FROM prompts WHERE canonical_path = ?1",
                    params![canonical_path],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            let prompt_id = match existing_id {
                Some(id) => {
                    tx.execute(
                        r#"
                        UPDATE prompts
                        SET name = ?2,
                            canonical_path = ?3,
                            updated_at = ?4
                        WHERE id = ?1
                        "#,
                        params![id, source.name, canonical_path, now],
                    )?;
                    id
                }
                None => {
                    let id = Uuid::new_v4().to_string();
                    tx.execute(
                        r#"
                        INSERT INTO prompts (id, name, canonical_path, created_at, updated_at)
                        VALUES (?1, ?2, ?3, ?4, ?4)
                        "#,
                        params![id, source.name, canonical_path, now],
                    )?;
                    id
                }
            };

            let existing_targets = target_paths_for_prompt(&tx, &prompt_id)?;
            tx.execute(
                "DELETE FROM prompt_distributions WHERE prompt_id = ?1",
                params![prompt_id],
            )?;
            for (agent, role, target_path) in distribution_rows(&source, &existing_targets)? {
                tx.execute(
                    r#"
                    INSERT INTO prompt_distributions (prompt_id, agent, role, target_path)
                    VALUES (?1, ?2, ?3, ?4)
                    "#,
                    params![prompt_id, agent, role, target_path],
                )?;
            }
        }

        let mut stmt = tx.prepare("SELECT id, canonical_path FROM prompts")?;
        let existing_rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let existing = existing_rows.collect::<Result<Vec<_>, _>>()?;
        drop(stmt);

        for (id, canonical_path) in existing {
            if !scanned_paths.contains(&canonical_path) {
                tx.execute("DELETE FROM prompts WHERE id = ?1", params![id])?;
            }
        }

        tx.commit()?;
        Ok(())
    }

    fn known_prompt_target_paths(&self) -> AppResult<BTreeSet<String>> {
        let conn = self.db.connection()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT target_path
            FROM prompt_distributions
            WHERE role = 'target' AND target_path IS NOT NULL
            "#,
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        rows.collect::<Result<BTreeSet<_>, _>>().map_err(Into::into)
    }

    fn get_prompt(&self, id: &str) -> AppResult<Prompt> {
        let conn = self.db.connection()?;
        conn.query_row(
            r#"
            SELECT id, name, canonical_path
            FROM prompts
            WHERE id = ?1
            "#,
            params![id],
            |row| prompt_from_row(row, &conn),
        )
        .optional()?
        .ok_or_else(|| AppError::Validation("prompt was not found".to_string()))
    }

    fn prompt_target_context(&self, id: &str) -> AppResult<PromptTargetContext> {
        let conn = self.db.connection()?;
        conn.query_row(
            r#"
            SELECT p.canonical_path, d.agent
            FROM prompts p
            JOIN prompt_distributions d ON d.prompt_id = p.id AND d.role = 'source'
            WHERE p.id = ?1
            "#,
            params![id],
            |row| {
                let canonical_path: String = row.get(0)?;
                Ok(PromptTargetContext {
                    canonical_path: PathBuf::from(canonical_path),
                    source_agent: row.get(1)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::Validation("prompt source was not found".to_string()))
    }

    fn prompt_canonical_path(&self, id: &str) -> AppResult<PathBuf> {
        let conn = self.db.connection()?;
        conn.query_row(
            "SELECT canonical_path FROM prompts WHERE id = ?1",
            params![id],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .map(PathBuf::from)
        .ok_or_else(|| AppError::Validation("prompt was not found".to_string()))
    }
}

fn discover_prompt_source(
    agent: &AgentCapabilitySurface,
    prompt_file: &Path,
    known_target_paths: &BTreeSet<String>,
) -> AppResult<Option<PromptSource>> {
    let metadata = match fs::symlink_metadata(prompt_file) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };

    let display_path = path_to_string(prompt_file, "prompt path")?;
    if metadata.file_type().is_symlink() || known_target_paths.contains(&display_path) {
        return Ok(None);
    }
    if !metadata.is_file() {
        return Ok(None);
    }

    Ok(Some(PromptSource {
        source_agent: agent.name,
        name: format!("{} Global Prompt", agent.name),
        canonical_path: prompt_file.canonicalize()?,
    }))
}

fn target_paths_for_prompt(
    tx: &rusqlite::Transaction<'_>,
    prompt_id: &str,
) -> rusqlite::Result<BTreeSet<String>> {
    let mut stmt = tx.prepare(
        r#"
        SELECT target_path
        FROM prompt_distributions
        WHERE prompt_id = ?1 AND role = 'target' AND target_path IS NOT NULL
        "#,
    )?;
    let rows = stmt.query_map([prompt_id], |row| row.get::<_, String>(0))?;
    rows.collect()
}

fn distribution_rows(
    source: &PromptSource,
    existing_targets: &BTreeSet<String>,
) -> AppResult<Vec<(&'static str, String, Option<String>)>> {
    let mut rows = Vec::new();
    for agent in agent_capability_surfaces() {
        if agent.name == source.source_agent {
            rows.push((agent.name, "source".to_string(), None));
            continue;
        }

        let target_path = target_path_for_agent(agent)?;
        let role = if let Some(target_path) = &target_path {
            if symlink_points_to(target_path, &source.canonical_path)?
                || existing_target_exists(existing_targets, target_path)?
            {
                "target"
            } else {
                "none"
            }
        } else {
            "none"
        };

        rows.push((
            agent.name,
            role.to_string(),
            if role == "target" {
                target_path
                    .as_ref()
                    .map(|path| path_to_string(path, "prompt target path"))
                    .transpose()?
            } else {
                None
            },
        ));
    }

    Ok(rows)
}

fn target_path_for_agent(agent: &AgentCapabilitySurface) -> AppResult<Option<PathBuf>> {
    let Some(prompt) = agent.prompt else {
        return Ok(None);
    };
    Ok(Some(paths::resolve_local_path(prompt.global_file)?))
}

fn existing_target_exists(
    existing_targets: &BTreeSet<String>,
    target_path: &Path,
) -> AppResult<bool> {
    let target_path = path_to_string(target_path, "prompt target path")?;
    Ok(existing_targets.contains(&target_path) && Path::new(&target_path).exists())
}

fn symlink_points_to(target_path: &Path, source_path: &Path) -> AppResult<bool> {
    let metadata = match fs::symlink_metadata(target_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    if !metadata.file_type().is_symlink() && !is_junction(target_path) {
        return Ok(false);
    }

    let Ok(resolved_target) = target_path.canonicalize() else {
        return Ok(false);
    };
    Ok(resolved_target == source_path.canonicalize()?)
}

fn prompt_from_row(row: &Row<'_>, conn: &rusqlite::Connection) -> rusqlite::Result<Prompt> {
    let id: String = row.get(0)?;
    Ok(Prompt {
        cells: prompt_cells(conn, &id)?,
        id,
        name: row.get(1)?,
        path: row.get(2)?,
    })
}

fn prompt_cells(
    conn: &rusqlite::Connection,
    prompt_id: &str,
) -> rusqlite::Result<BTreeMap<String, String>> {
    let mut cells = empty_cells();
    let mut stmt = conn.prepare(
        r#"
        SELECT agent, role
        FROM prompt_distributions
        WHERE prompt_id = ?1
        "#,
    )?;
    let rows = stmt.query_map([prompt_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;

    for row in rows {
        let (agent, role) = row?;
        cells.insert(agent, role);
    }

    Ok(cells)
}

fn empty_cells() -> BTreeMap<String, String> {
    agent_capability_surfaces()
        .iter()
        .map(|agent| (agent.name.to_string(), "none".to_string()))
        .collect()
}

fn source_index(prompt: &Prompt) -> usize {
    agent_capability_surfaces()
        .iter()
        .enumerate()
        .find_map(|(index, agent)| {
            (prompt.cells.get(agent.name).map(String::as_str) == Some("source")).then_some(index)
        })
        .unwrap_or(usize::MAX)
}

fn source_index_for_agent(agent_name: &str) -> usize {
    agent_capability_surfaces()
        .iter()
        .position(|agent| agent.name == agent_name)
        .unwrap_or(usize::MAX)
}

fn agent_by_name(name: &str) -> AppResult<&'static AgentCapabilitySurface> {
    capability_by_name(name).ok_or_else(|| AppError::Validation("invalid agent".to_string()))
}

fn path_to_string(path: &Path, label: &str) -> AppResult<String> {
    paths::path_to_string(path, label)
}

fn required_trimmed<'a>(value: &'a str, label: &str) -> AppResult<&'a str> {
    let value = value.trim();
    if value.is_empty() {
        Err(AppError::Validation(format!("{label} is required")))
    } else {
        Ok(value)
    }
}

#[cfg(target_os = "macos")]
fn open_path(path: &Path) -> AppResult<()> {
    Command::new("open").arg(path).spawn()?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn open_path(path: &Path) -> AppResult<()> {
    Command::new("explorer").arg(path).spawn()?;
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn open_path(path: &Path) -> AppResult<()> {
    Command::new("xdg-open").arg(path).spawn()?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn reveal_path(path: &Path) -> AppResult<()> {
    Command::new("open").arg("-R").arg(path).spawn()?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn reveal_path(path: &Path) -> AppResult<()> {
    Command::new("explorer")
        .arg(format!("/select,{}", path.display()))
        .spawn()?;
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn reveal_path(path: &Path) -> AppResult<()> {
    let target = path.parent().unwrap_or(path);
    Command::new("xdg-open").arg(target).spawn()?;
    Ok(())
}

fn now_epoch_seconds() -> AppResult<i64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| AppError::Internal(error.to_string()))?
        .as_secs() as i64)
}
