use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    database::Database,
    error::{AppError, AppResult},
    services::agent_capabilities::{agent_capability_surfaces, AgentCapabilitySurface},
    services::distribution::{self, MatrixSource},
    services::paths::{self, path_to_string},
    services::symlink::{create_managed_file_link, remove_managed_file_link_if_present},
    services::system_open::{open_path, reveal_path},
    services::util::{now_epoch_seconds, require_agent, required_trimmed},
};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Prompt {
    pub id: String,
    pub name: String,
    pub scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    pub path: String,
    pub content: String,
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
struct ProjectRoot {
    id: String,
    name: String,
    path: PathBuf,
}

#[derive(Debug, Clone)]
struct PromptSource {
    source_agent: &'static str,
    scope: String,
    project_id: Option<String>,
    project_path: Option<PathBuf>,
    name: String,
    canonical_path: PathBuf,
}

#[derive(Debug, Clone)]
struct PromptTargetContext {
    scope: String,
    project_path: Option<PathBuf>,
    canonical_path: PathBuf,
    source_agent: String,
}

/// A scanned Prompt source paired with the already-known target paths, so `matrix_rows` can
/// honour a previously-recorded target that still exists on disk (Prompt-specific signal).
struct PromptMatrixSource<'a> {
    source: &'a PromptSource,
    existing_targets: &'a BTreeSet<String>,
}

impl MatrixSource for PromptMatrixSource<'_> {
    fn source_agent(&self) -> &str {
        self.source.source_agent
    }

    fn canonical_path(&self) -> &Path {
        &self.source.canonical_path
    }

    fn target_path_for(&self, agent: &AgentCapabilitySurface) -> AppResult<Option<PathBuf>> {
        target_path_for_parts(
            &self.source.scope,
            self.source.project_path.as_deref(),
            agent,
        )
    }

    fn target_path_label(&self) -> &'static str {
        "prompt target path"
    }

    fn is_existing_target(&self, target_path: &Path) -> AppResult<bool> {
        existing_target_exists(self.existing_targets, target_path)
    }
}

impl PromptService {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    pub fn list_prompts(&self) -> AppResult<Vec<Prompt>> {
        let conn = self.db.connection()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT p.id, p.name, p.scope, p.project_id, p.canonical_path
            FROM prompts p
            LEFT JOIN projects project ON project.id = p.project_id
            JOIN prompt_distributions source
                ON source.prompt_id = p.id AND source.role = 'source'
            ORDER BY
                CASE p.scope WHEN 'global' THEN 0 ELSE 1 END,
                CASE WHEN p.scope = 'project' THEN project.sort_index IS NULL ELSE 0 END,
                CASE WHEN p.scope = 'project' THEN project.sort_index END,
                CASE WHEN p.scope = 'project' THEN project.created_at END,
                CASE WHEN p.scope = 'project' THEN project.name END,
                CASE source.agent
                    WHEN 'Generic Agent' THEN 0
                    WHEN 'Claude Code' THEN 1
                    WHEN 'CodeX' THEN 2
                    WHEN 'Copilot' THEN 3
                    WHEN 'OpenCode' THEN 4
                    ELSE 5
                END,
                p.name,
                p.canonical_path
            "#,
        )?;
        let rows = stmt.query_map([], |row| prompt_from_row(row, &conn))?;
        let mut prompts = rows.collect::<Result<Vec<_>, _>>()?;
        for prompt in &mut prompts {
            prompt.content = fs::read_to_string(&prompt.path)?;
        }
        Ok(prompts)
    }

    pub fn scan_prompts(&self) -> AppResult<Vec<Prompt>> {
        let projects = self.list_project_roots()?;
        let known_target_paths = self.known_prompt_target_paths()?;
        let mut sources = Vec::new();

        for agent in agent_capability_surfaces() {
            let Some(prompt) = agent.prompt else {
                continue;
            };
            let prompt_file = paths::resolve_local_path(prompt.global_file)?;
            if let Some(source) = discover_prompt_source(
                agent,
                "global",
                None,
                None,
                None,
                &prompt_file,
                &known_target_paths,
            )? {
                sources.push(source);
            }
        }

        for project in &projects {
            for agent in agent_capability_surfaces() {
                let Some(project_file) = agent.prompt.and_then(|prompt| prompt.project_file) else {
                    continue;
                };
                let prompt_file = project.path.join(project_file);
                if let Some(source) = discover_prompt_source(
                    agent,
                    "project",
                    Some(project.id.clone()),
                    Some(project.name.as_str()),
                    Some(project.path.clone()),
                    &prompt_file,
                    &known_target_paths,
                )? {
                    sources.push(source);
                }
            }
        }

        sources.sort_by(|left, right| {
            left.scope
                .cmp(&right.scope)
                .then_with(|| left.project_id.cmp(&right.project_id))
                .then_with(|| {
                    source_index_for_agent(left.source_agent)
                        .cmp(&source_index_for_agent(right.source_agent))
                })
                .then_with(|| left.canonical_path.cmp(&right.canonical_path))
        });
        self.replace_scanned_sources(sources)?;
        self.list_prompts()
    }

    pub fn set_prompt_target(&self, input: SetPromptTargetInput) -> AppResult<Prompt> {
        let prompt_id = required_trimmed(&input.prompt_id, "prompt id")?;
        let target_agent = require_agent(required_trimmed(&input.agent, "agent")?)?;
        if target_agent.prompt.is_none() {
            return Err(AppError::Validation(
                "agent does not support prompt targets".to_string(),
            ));
        }

        let context = self.prompt_target_context(prompt_id)?;
        let source_agent = require_agent(&context.source_agent)?;
        if source_agent.name == target_agent.name {
            return Err(AppError::Validation(
                "source agent cannot be toggled as a target".to_string(),
            ));
        }

        let target_path = target_path_for_parts(
            &context.scope,
            context.project_path.as_deref(),
            target_agent,
        )?
        .ok_or_else(|| {
            AppError::Validation("agent does not support prompt targets in this scope".to_string())
        })?;

        distribution::write_target(
            &self.db,
            "prompt_distributions",
            "prompt_id",
            prompt_id,
            target_agent.name,
            input.enabled,
            &context.canonical_path,
            &target_path,
            "prompt target path",
            create_managed_file_link,
            remove_managed_file_link_if_present,
        )?;

        self.get_prompt(prompt_id)
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

    fn list_project_roots(&self) -> AppResult<Vec<ProjectRoot>> {
        let conn = self.db.connection()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT id, name, path
            FROM projects
            WHERE status = 'active'
            ORDER BY sort_index IS NULL, sort_index, created_at, name
            "#,
        )?;
        let rows = stmt.query_map([], |row| {
            let path: String = row.get(2)?;
            Ok(ProjectRoot {
                id: row.get(0)?,
                name: row.get(1)?,
                path: PathBuf::from(path),
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
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
                            scope = ?3,
                            project_id = ?4,
                            canonical_path = ?5,
                            updated_at = ?6
                        WHERE id = ?1
                        "#,
                        params![
                            id,
                            source.name,
                            source.scope,
                            source.project_id,
                            canonical_path,
                            now
                        ],
                    )?;
                    id
                }
                None => {
                    let id = Uuid::new_v4().to_string();
                    tx.execute(
                        r#"
                        INSERT INTO prompts (
                            id, name, scope, project_id, canonical_path, created_at, updated_at
                        )
                        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
                        "#,
                        params![
                            id,
                            source.name,
                            source.scope,
                            source.project_id,
                            canonical_path,
                            now
                        ],
                    )?;
                    id
                }
            };

            let existing_targets = target_paths_for_prompt(&tx, &prompt_id)?;
            tx.execute(
                "DELETE FROM prompt_distributions WHERE prompt_id = ?1",
                params![prompt_id],
            )?;
            let matrix_source = PromptMatrixSource {
                source: &source,
                existing_targets: &existing_targets,
            };
            for (agent, role, target_path) in distribution::matrix_rows(&matrix_source)? {
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
        let mut prompt = conn
            .query_row(
                r#"
            SELECT id, name, scope, project_id, canonical_path
            FROM prompts
            WHERE id = ?1
            "#,
                params![id],
                |row| prompt_from_row(row, &conn),
            )
            .optional()?
            .ok_or_else(|| AppError::Validation("prompt was not found".to_string()))?;
        prompt.content = fs::read_to_string(&prompt.path)?;
        Ok(prompt)
    }

    fn prompt_target_context(&self, id: &str) -> AppResult<PromptTargetContext> {
        let conn = self.db.connection()?;
        conn.query_row(
            r#"
            SELECT p.scope, project.path, p.canonical_path, d.agent
            FROM prompts p
            LEFT JOIN projects project ON project.id = p.project_id
            JOIN prompt_distributions d ON d.prompt_id = p.id AND d.role = 'source'
            WHERE p.id = ?1
            "#,
            params![id],
            |row| {
                let project_path: Option<String> = row.get(1)?;
                let canonical_path: String = row.get(2)?;
                Ok(PromptTargetContext {
                    scope: row.get(0)?,
                    project_path: project_path.map(PathBuf::from),
                    canonical_path: PathBuf::from(canonical_path),
                    source_agent: row.get(3)?,
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
    scope: &str,
    project_id: Option<String>,
    project_name: Option<&str>,
    project_path: Option<PathBuf>,
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

    let file_name = prompt_file
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| AppError::Validation("prompt file has no file name".to_string()))?
        .to_string();
    let name = match scope {
        "global" => file_name,
        "project" => format!(
            "{} · {}",
            project_name.ok_or_else(|| {
                AppError::Validation("project prompt source has no project name".to_string())
            })?,
            file_name
        ),
        _ => {
            return Err(AppError::Validation(format!(
                "unsupported prompt scope: {scope}"
            )))
        }
    };

    Ok(Some(PromptSource {
        source_agent: agent.name,
        scope: scope.to_string(),
        project_id,
        project_path,
        name,
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

fn target_path_for_parts(
    scope: &str,
    project_path: Option<&Path>,
    agent: &AgentCapabilitySurface,
) -> AppResult<Option<PathBuf>> {
    let Some(prompt) = agent.prompt else {
        return Ok(None);
    };
    match scope {
        "global" => Ok(Some(paths::resolve_local_path(prompt.global_file)?)),
        "project" => {
            let Some(project_file) = prompt.project_file else {
                return Ok(None);
            };
            let project_path = project_path.ok_or_else(|| {
                AppError::Validation("project prompt has no project path".to_string())
            })?;
            Ok(Some(project_path.join(project_file)))
        }
        _ => Err(AppError::Validation(format!(
            "unsupported prompt scope: {scope}"
        ))),
    }
}

fn existing_target_exists(
    existing_targets: &BTreeSet<String>,
    target_path: &Path,
) -> AppResult<bool> {
    let target_path = path_to_string(target_path, "prompt target path")?;
    Ok(existing_targets.contains(&target_path) && Path::new(&target_path).exists())
}

fn prompt_from_row(row: &Row<'_>, conn: &rusqlite::Connection) -> rusqlite::Result<Prompt> {
    let id: String = row.get(0)?;
    Ok(Prompt {
        cells: distribution::cells(conn, "prompt_distributions", "prompt_id", &id)?,
        id,
        name: row.get(1)?,
        scope: row.get(2)?,
        project_id: row.get(3)?,
        path: row.get(4)?,
        content: String::new(),
    })
}

fn source_index_for_agent(agent_name: &str) -> usize {
    agent_capability_surfaces()
        .iter()
        .position(|agent| agent.name == agent_name)
        .unwrap_or(usize::MAX)
}
