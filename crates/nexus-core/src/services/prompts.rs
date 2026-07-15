use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use rusqlite::{params, OptionalExtension, Row, Transaction};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    database::Database,
    error::{AppError, AppResult},
    services::agent_capabilities::{
        agent_by_name, agent_capability_surfaces, agent_order_index, AgentCapabilitySurface,
    },
    services::distribution::{self, MatrixSource},
    services::paths::{self, path_to_string},
    services::projects::{self, parse_dir_list},
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MovePromptSourceInput {
    pub prompt_id: String,
    pub agent: String,
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
    extra_prompt_files: Vec<String>,
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
    project_id: Option<String>,
    project_path: Option<PathBuf>,
    project_name: Option<String>,
    canonical_path: PathBuf,
    source_agent: String,
}

#[derive(Debug, Clone)]
struct ExtraPromptMove {
    project_id: String,
    old_rel: String,
    new_rel: String,
}

#[derive(Debug, Clone)]
pub(crate) struct PromptRelocationMetadata {
    new_name: String,
    extra_prompt_move: Option<ExtraPromptMove>,
}

#[derive(Debug, Clone)]
struct PromptListRow {
    prompt: Prompt,
    source_agent: String,
    project_sort_index: Option<i64>,
    project_created_at: Option<i64>,
    project_name: Option<String>,
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
        let source_agent = agent_by_name(self.source.source_agent).ok_or_else(|| {
            AppError::Validation(format!(
                "unknown source agent: {}",
                self.source.source_agent
            ))
        })?;
        prompt_target_path(
            &self.source.scope,
            self.source.project_path.as_deref(),
            &self.source.canonical_path,
            source_agent,
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

impl distribution::SourceRelocationAdapter for PromptService {
    type Metadata = PromptRelocationMetadata;

    fn database(&self) -> &Database {
        &self.db
    }

    fn plan_relocation(
        &self,
        asset_id: &str,
        target_agent: &'static AgentCapabilitySurface,
    ) -> AppResult<distribution::RelocationPlan<Self::Metadata>> {
        if target_agent.prompt.is_none() {
            return Err(AppError::Validation(
                "agent does not support prompt targets".to_string(),
            ));
        }
        let context = self.prompt_target_context(asset_id)?;
        let source_agent = require_agent(&context.source_agent)?;
        if source_agent.name == target_agent.name {
            return Ok(distribution::RelocationPlan::Unchanged);
        }
        let new_canonical_path = prompt_target_path(
            &context.scope,
            context.project_path.as_deref(),
            &context.canonical_path,
            source_agent,
            target_agent,
        )?
        .ok_or_else(|| {
            AppError::Validation("agent does not support prompt targets in this scope".to_string())
        })?;
        let extra_prompt_move = if context.scope == "project" {
            match (&context.project_id, context.project_path.as_deref()) {
                (Some(project_id), Some(project_path)) => Some(ExtraPromptMove {
                    project_id: project_id.clone(),
                    old_rel: project_relative_prompt_path(project_path, &context.canonical_path)?,
                    new_rel: project_relative_prompt_path(project_path, &new_canonical_path)?,
                }),
                _ => None,
            }
        } else {
            None
        };
        let new_name = prompt_display_name(
            &context.scope,
            context.project_name.as_deref(),
            &new_canonical_path,
        )?;

        Ok(distribution::RelocationPlan::Move(
            distribution::PreparedSourceMove {
                asset_id: asset_id.to_string(),
                storage: distribution::DistributionStorage::Prompt,
                old_source_agent: source_agent.name,
                new_source_agent: target_agent.name,
                old_canonical_path: context.canonical_path,
                new_canonical_path,
                placement_kind: distribution::PlacementKind::File,
                metadata: PromptRelocationMetadata {
                    new_name,
                    extra_prompt_move,
                },
            },
        ))
    }

    fn persist_asset_move(
        &self,
        tx: &Transaction<'_>,
        movement: &distribution::PreparedSourceMove<Self::Metadata>,
        now: i64,
    ) -> AppResult<()> {
        let new_canonical_path = path_to_string(&movement.new_canonical_path, "prompt path")?;
        let changed = tx.execute(
            r#"
            UPDATE prompts
            SET name = ?2,
                canonical_path = ?3,
                updated_at = ?4
            WHERE id = ?1
            "#,
            params![
                movement.asset_id,
                movement.metadata.new_name,
                new_canonical_path,
                now
            ],
        )?;
        if changed == 0 {
            return Err(AppError::Validation("prompt was not found".to_string()));
        }
        if let Some(extra_prompt_move) = &movement.metadata.extra_prompt_move {
            update_project_extra_prompt_files_for_move(tx, extra_prompt_move, now)?;
        }
        Ok(())
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
            SELECT
                p.id,
                p.name,
                p.scope,
                p.project_id,
                p.canonical_path,
                source.agent,
                project.sort_index,
                project.created_at,
                project.name
            FROM prompts p
            LEFT JOIN projects project ON project.id = p.project_id
            JOIN prompt_distributions source
                ON source.prompt_id = p.id AND source.role = 'source'
            "#,
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(PromptListRow {
                prompt: prompt_from_row(row, &conn)?,
                source_agent: row.get(5)?,
                project_sort_index: row.get(6)?,
                project_created_at: row.get(7)?,
                project_name: row.get(8)?,
            })
        })?;
        let mut rows = rows.collect::<Result<Vec<_>, _>>()?;
        rows.sort_by(compare_prompt_list_rows);

        for row in &mut rows {
            row.prompt.content = fs::read_to_string(&row.prompt.path)?;
            row.prompt.path = paths::collapse_home(&row.prompt.path);
        }
        Ok(rows.into_iter().map(|row| row.prompt).collect())
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

            // Extra Prompt Files: each registered file is owned by the Agent whose
            // prompt glob its name matches (AGENTS*.md → Generic Agent, CLAUDE*.md →
            // Claude Code) and scanned in that Agent's namespace.
            for extra in &project.extra_prompt_files {
                let Some(agent) = prompt_agent_for_file(extra) else {
                    continue;
                };
                let prompt_file = project.path.join(extra);
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

    pub fn move_prompt_source(&self, input: MovePromptSourceInput) -> AppResult<Prompt> {
        let prompt_id = required_trimmed(&input.prompt_id, "prompt id")?;
        let target_agent = require_agent(required_trimmed(&input.agent, "agent")?)?;
        distribution::relocate_source(self, prompt_id, target_agent)?;
        self.get_prompt(prompt_id)
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

        let target_path = prompt_target_path(
            &context.scope,
            context.project_path.as_deref(),
            &context.canonical_path,
            source_agent,
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
            None,
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
            SELECT id, name, path, extra_prompt_files
            FROM projects
            WHERE status = 'active'
            ORDER BY sort_index IS NULL, sort_index, created_at, name
            "#,
        )?;
        let rows = stmt.query_map([], |row| {
            let path: String = row.get(2)?;
            let extra: String = row.get(3)?;
            Ok(ProjectRoot {
                id: row.get(0)?,
                name: row.get(1)?,
                path: PathBuf::from(path),
                extra_prompt_files: parse_dir_list(&extra),
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
        prompt.path = paths::collapse_home(&prompt.path);
        Ok(prompt)
    }

    fn prompt_target_context(&self, id: &str) -> AppResult<PromptTargetContext> {
        let conn = self.db.connection()?;
        conn.query_row(
            r#"
            SELECT p.scope, p.project_id, project.path, project.name, p.canonical_path, d.agent
            FROM prompts p
            LEFT JOIN projects project ON project.id = p.project_id
            JOIN prompt_distributions d ON d.prompt_id = p.id AND d.role = 'source'
            WHERE p.id = ?1
            "#,
            params![id],
            |row| {
                let project_id: Option<String> = row.get(1)?;
                let project_path: Option<String> = row.get(2)?;
                let project_name: Option<String> = row.get(3)?;
                let canonical_path: String = row.get(4)?;
                Ok(PromptTargetContext {
                    scope: row.get(0)?,
                    project_id,
                    project_path: project_path.map(PathBuf::from),
                    project_name,
                    canonical_path: PathBuf::from(canonical_path),
                    source_agent: row.get(5)?,
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

fn prompt_display_name(
    scope: &str,
    project_name: Option<&str>,
    prompt_file: &Path,
) -> AppResult<String> {
    let file_name = prompt_file
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| AppError::Validation("prompt file has no file name".to_string()))?;
    match scope {
        "global" => Ok(file_name.to_string()),
        "project" => Ok(format!(
            "{} · {}",
            project_name.ok_or_else(|| {
                AppError::Validation("project prompt source has no project name".to_string())
            })?,
            file_name
        )),
        _ => Err(AppError::Validation(format!(
            "unsupported prompt scope: {scope}"
        ))),
    }
}

fn project_relative_prompt_path(project_path: &Path, prompt_file: &Path) -> AppResult<String> {
    let rel = prompt_file
        .strip_prefix(project_path)
        .unwrap_or(prompt_file);
    Ok(projects::normalize_custom_dir(&path_to_string(
        rel,
        "project prompt path",
    )?))
}

fn update_project_extra_prompt_files_for_move(
    tx: &rusqlite::Transaction<'_>,
    prompt_move: &ExtraPromptMove,
    now: i64,
) -> AppResult<()> {
    let current = tx
        .query_row(
            "SELECT extra_prompt_files FROM projects WHERE id = ?1",
            params![prompt_move.project_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    let Some(current) = current else {
        return Ok(());
    };

    let old_norm = projects::normalize_custom_dir(&prompt_move.old_rel);
    let mut changed = false;
    let mut seen = BTreeSet::new();
    let mut next = Vec::new();
    for file in parse_dir_list(&current) {
        let candidate = if projects::normalize_custom_dir(&file) == old_norm {
            changed = true;
            prompt_move.new_rel.clone()
        } else {
            file
        };
        let identity = projects::normalize_custom_dir(&candidate);
        if seen.insert(identity) {
            next.push(candidate);
        }
    }

    if changed {
        tx.execute(
            "UPDATE projects SET extra_prompt_files = ?2, updated_at = ?3 WHERE id = ?1",
            params![prompt_move.project_id, next.join("\n"), now],
        )?;
    }

    Ok(())
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

    let name = prompt_display_name(scope, project_name, prompt_file)?;

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

/// The Agent that owns an extra prompt file, by matching its basename against each
/// Agent's prompt glob (`AGENTS*.md` → Generic Agent, `CLAUDE*.md` → Claude Code).
fn prompt_agent_for_file(file: &str) -> Option<&'static AgentCapabilitySurface> {
    let base = file.rsplit(['/', '\\']).next().unwrap_or(file);
    if !base.ends_with(".md") {
        return None;
    }
    agent_capability_surfaces().iter().find(|agent| {
        prompt_stem(agent)
            .map(|stem| base.starts_with(stem))
            .unwrap_or(false)
    })
}

/// The prompt-file stem for an Agent (`AGENTS.md` → `AGENTS`), if it has a project file.
fn prompt_stem(agent: &AgentCapabilitySurface) -> Option<&'static str> {
    agent
        .prompt
        .and_then(|prompt| prompt.project_file)
        .and_then(|file| file.strip_suffix(".md"))
}

/// Resolve a prompt's target path on `target_agent`. Global prompts map to the
/// Agent's global file; project prompts swap the source Agent's prompt-file stem for
/// the target Agent's stem, keeping any directory prefix and the suffix between the
/// stem and `.md` (so `AGENTS.md` → `CLAUDE.md`, `AGENTS.local.md` → `CLAUDE.local.md`).
fn prompt_target_path(
    scope: &str,
    project_path: Option<&Path>,
    source_canonical: &Path,
    source_agent: &AgentCapabilitySurface,
    target_agent: &AgentCapabilitySurface,
) -> AppResult<Option<PathBuf>> {
    let Some(prompt) = target_agent.prompt else {
        return Ok(None);
    };
    match scope {
        "global" => Ok(Some(paths::resolve_local_path(prompt.global_file)?)),
        "project" => {
            let (Some(source_stem), Some(target_stem)) =
                (prompt_stem(source_agent), prompt_stem(target_agent))
            else {
                return Ok(None);
            };
            let project_path = project_path.ok_or_else(|| {
                AppError::Validation("project prompt has no project path".to_string())
            })?;

            let rel = source_canonical
                .strip_prefix(project_path)
                .unwrap_or(source_canonical);
            let file_name = rel
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| AppError::Validation("prompt file has no file name".to_string()))?;
            let suffix = file_name
                .strip_prefix(source_stem)
                .and_then(|rest| rest.strip_suffix(".md"))
                .ok_or_else(|| {
                    AppError::Validation(format!(
                        "prompt file does not match its source agent glob: {file_name}"
                    ))
                })?;
            let target_name = format!("{target_stem}{suffix}.md");
            let target_rel = match rel.parent() {
                Some(parent) if !parent.as_os_str().is_empty() => parent.join(target_name),
                _ => PathBuf::from(target_name),
            };
            Ok(Some(project_path.join(target_rel)))
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

fn compare_prompt_list_rows(left: &PromptListRow, right: &PromptListRow) -> Ordering {
    scope_rank(&left.prompt.scope)
        .cmp(&scope_rank(&right.prompt.scope))
        .then_with(|| project_sort_missing(left).cmp(&project_sort_missing(right)))
        .then_with(|| left.project_sort_index.cmp(&right.project_sort_index))
        .then_with(|| left.project_created_at.cmp(&right.project_created_at))
        .then_with(|| left.project_name.cmp(&right.project_name))
        .then_with(|| {
            source_index_for_agent(&left.source_agent)
                .cmp(&source_index_for_agent(&right.source_agent))
        })
        .then_with(|| left.prompt.name.cmp(&right.prompt.name))
        .then_with(|| left.prompt.path.cmp(&right.prompt.path))
}

fn scope_rank(scope: &str) -> u8 {
    match scope {
        "global" => 0,
        _ => 1,
    }
}

fn project_sort_missing(row: &PromptListRow) -> u8 {
    if row.prompt.scope == "project" && row.project_sort_index.is_none() {
        1
    } else {
        0
    }
}

fn source_index_for_agent(agent_name: &str) -> usize {
    agent_order_index(agent_name).unwrap_or(usize::MAX)
}
