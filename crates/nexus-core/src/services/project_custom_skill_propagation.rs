use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    database::Database,
    error::{AppError, AppResult},
    services::{
        agent_capabilities::{agent_by_name, agent_capability_surfaces},
        app_config::AppConfigService,
        distribution,
        paths::{self, path_to_string},
        projects,
        skills::{
            AgentCellRole, PlacementCellRole, ProjectCustomDestinationState,
            ProjectCustomSkillDestination, ProjectCustomSkillIntent,
            ProjectCustomSkillMutationResult, ProjectRef, SkillContext, SkillRow, SkillSummary,
            SOURCE_KIND_AGENT, SOURCE_KIND_PROJECT_CUSTOM,
        },
        symlink::{create_managed_directory_link, remove_managed_directory_link_if_present},
        util::{now_epoch_seconds, required_trimmed},
    },
};

#[derive(Debug)]
struct CatalogRecord {
    id: String,
    name: String,
    scope: String,
    project_id: Option<String>,
    project_name: Option<String>,
    desc: String,
    canonical_path: String,
    disabled: bool,
    source_kind: String,
    source_agent: Option<String>,
}

/// Build the authoritative Skill catalog from one SQLite snapshot. Both normal
/// reads and Project-custom mutation transactions use this function so identity,
/// eager destinations, and incoming projections cannot drift between callers.
pub(crate) fn catalog_from_connection(conn: &Connection) -> AppResult<Vec<SkillRow>> {
    let mut stmt = conn.prepare(
        r#"
        SELECT
            s.id,
            s.name,
            s.scope,
            s.project_id,
            p.name,
            COALESCE(s.description, ''),
            s.canonical_path,
            s.disabled,
            s.source_kind,
            s.source_agent
        FROM skills s
        LEFT JOIN projects p ON p.id = s.project_id
        ORDER BY
            s.scope,
            p.sort_index IS NULL,
            p.sort_index,
            p.created_at,
            p.name,
            s.name,
            s.canonical_path
        "#,
    )?;
    let records = stmt
        .query_map([], |row| {
            Ok(CatalogRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                scope: row.get(2)?,
                project_id: row.get(3)?,
                project_name: row.get(4)?,
                desc: row.get(5)?,
                canonical_path: row.get(6)?,
                disabled: row.get::<_, i64>(7)? != 0,
                source_kind: row.get(8)?,
                source_agent: row.get(9)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(stmt);

    let effective_projects = projects::list_effective_projects(conn)?;
    let mut canonical_rows = Vec::with_capacity(records.len());
    let mut incoming_rows = Vec::new();

    for record in records {
        let summary = SkillSummary {
            skill_id: record.id.clone(),
            name: record.name.clone(),
            desc: record.desc.clone(),
            path: paths::collapse_home(&record.canonical_path),
            disabled: record.disabled,
        };

        match record.source_kind.as_str() {
            SOURCE_KIND_AGENT => {
                let source_agent = record.source_agent.ok_or_else(|| {
                    AppError::Internal(format!(
                        "Agent-sourced Skill has no source Agent: {}",
                        record.id
                    ))
                })?;
                let cells = agent_cells(conn, &record.id)?;
                if cells
                    .values()
                    .filter(|role| **role == AgentCellRole::Source)
                    .count()
                    != 1
                    || cells.get(&source_agent) != Some(&AgentCellRole::Source)
                {
                    return Err(AppError::Internal(format!(
                        "Agent-sourced Skill must have exactly one source cell: {}",
                        record.id
                    )));
                }
                let context = match record.scope.as_str() {
                    "global" => SkillContext::Global,
                    "project" => SkillContext::Project {
                        project: ProjectRef {
                            id: record.project_id.ok_or_else(|| {
                                AppError::Internal(format!(
                                    "Project Skill has no Project: {}",
                                    record.id
                                ))
                            })?,
                            name: record.project_name.ok_or_else(|| {
                                AppError::Internal(format!(
                                    "Project Skill source Project was not found: {}",
                                    record.id
                                ))
                            })?,
                        },
                    },
                    other => {
                        return Err(AppError::Internal(format!(
                            "unknown Skill scope {other}: {}",
                            record.id
                        )))
                    }
                };
                canonical_rows.push(SkillRow::AgentCanonical {
                    row_key: record.id,
                    skill: summary,
                    context,
                    source_agent,
                    cells,
                });
            }
            SOURCE_KIND_PROJECT_CUSTOM => {
                let source_project = ProjectRef {
                    id: record.project_id.ok_or_else(|| {
                        AppError::Internal(format!(
                            "Project custom Skill has no source Project: {}",
                            record.id
                        ))
                    })?,
                    name: record.project_name.ok_or_else(|| {
                        AppError::Internal(format!(
                            "Project custom Skill source Project was not found: {}",
                            record.id
                        ))
                    })?,
                };
                let mut destinations = vec![ProjectCustomDestinationState::Global {
                    cells: global_placement_cells(conn, &record.id)?,
                }];

                for project in &effective_projects {
                    let project_ref = ProjectRef {
                        id: project.id.clone(),
                        name: project.name.clone(),
                    };
                    let cells = project_placement_cells(conn, &record.id, &project.id)?;
                    if cells
                        .values()
                        .any(|role| *role == PlacementCellRole::Target)
                    {
                        incoming_rows.push(SkillRow::ProjectCustomIncoming {
                            row_key: format!("{}::project::{}", record.id, project.id),
                            skill: summary.clone(),
                            source_project: source_project.clone(),
                            target_project: project_ref.clone(),
                            cells: cells.clone(),
                        });
                    }
                    destinations.push(ProjectCustomDestinationState::Project {
                        project: project_ref,
                        cells,
                    });
                }

                canonical_rows.push(SkillRow::ProjectCustomCanonical {
                    row_key: record.id,
                    skill: summary,
                    source_project,
                    destinations,
                });
            }
            other => {
                return Err(AppError::Internal(format!(
                    "unknown Skill source kind {other}: {}",
                    record.id
                )))
            }
        }
    }

    canonical_rows.extend(incoming_rows);
    Ok(canonical_rows)
}

fn empty_agent_cells() -> BTreeMap<String, AgentCellRole> {
    agent_capability_surfaces()
        .iter()
        .filter(|agent| agent.skill.is_some())
        .map(|agent| (agent.name.to_string(), AgentCellRole::None))
        .collect()
}

fn empty_placement_cells() -> BTreeMap<String, PlacementCellRole> {
    agent_capability_surfaces()
        .iter()
        .filter(|agent| agent.skill.is_some())
        .map(|agent| (agent.name.to_string(), PlacementCellRole::None))
        .collect()
}

fn agent_cells(conn: &Connection, skill_id: &str) -> AppResult<BTreeMap<String, AgentCellRole>> {
    let mut cells = empty_agent_cells();
    let mut stmt =
        conn.prepare("SELECT agent, role FROM skill_distributions WHERE skill_id = ?1")?;
    let rows = stmt.query_map([skill_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (agent, role) = row?;
        let role = match role.as_str() {
            "source" => AgentCellRole::Source,
            "target" => AgentCellRole::Target,
            "none" => AgentCellRole::None,
            other => {
                return Err(AppError::Internal(format!(
                    "unknown Agent Skill cell role: {other}"
                )))
            }
        };
        cells.insert(agent, role);
    }
    Ok(cells)
}

fn global_placement_cells(
    conn: &Connection,
    skill_id: &str,
) -> AppResult<BTreeMap<String, PlacementCellRole>> {
    let mut cells = empty_placement_cells();
    let mut stmt =
        conn.prepare("SELECT agent, role FROM skill_distributions WHERE skill_id = ?1")?;
    let rows = stmt.query_map([skill_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    overlay_placement_cells(&mut cells, rows)?;
    Ok(cells)
}

fn project_placement_cells(
    conn: &Connection,
    skill_id: &str,
    project_id: &str,
) -> AppResult<BTreeMap<String, PlacementCellRole>> {
    let mut cells = empty_placement_cells();
    let mut stmt = conn.prepare(
        r#"
        SELECT agent, role
        FROM skill_project_distributions
        WHERE skill_id = ?1 AND target_project_id = ?2
        "#,
    )?;
    let rows = stmt.query_map(params![skill_id, project_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    overlay_placement_cells(&mut cells, rows)?;
    Ok(cells)
}

fn overlay_placement_cells(
    cells: &mut BTreeMap<String, PlacementCellRole>,
    rows: rusqlite::MappedRows<
        '_,
        impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<(String, String)>,
    >,
) -> AppResult<()> {
    for row in rows {
        let (agent, role) = row?;
        let role = match role.as_str() {
            "target" => PlacementCellRole::Target,
            "none" => PlacementCellRole::None,
            "source" => {
                return Err(AppError::Internal(
                    "Project custom Skill cells cannot contain source".to_string(),
                ))
            }
            other => {
                return Err(AppError::Internal(format!(
                    "unknown Project custom Skill cell role: {other}"
                )))
            }
        };
        cells.insert(agent, role);
    }
    Ok(())
}

#[derive(Debug)]
struct IntentContext {
    skill_id: String,
    canonical_path: PathBuf,
    destination: DestinationContext,
}

#[derive(Debug)]
enum DestinationContext {
    Global,
    Project { project_id: String, root: PathBuf },
}

impl DestinationContext {
    fn kind(&self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Project { .. } => "project",
        }
    }

    fn project_id(&self) -> Option<&str> {
        match self {
            Self::Global => None,
            Self::Project { project_id, .. } => Some(project_id),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
enum StepAction {
    Create,
    Remove,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PlacementStep {
    action: StepAction,
    agent: String,
    path: String,
    #[serde(skip)]
    path_buf: PathBuf,
    #[serde(skip)]
    execute: bool,
    #[serde(skip)]
    missing_parent_dirs: Vec<PathBuf>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FailedCompensation {
    step: PlacementStep,
    error: String,
}

pub(crate) fn apply_intent(
    db: &Database,
    app_config: &AppConfigService,
    intent: ProjectCustomSkillIntent,
) -> AppResult<ProjectCustomSkillMutationResult> {
    let context = load_intent_context(db, &intent)?;
    let current = current_agents(db, &context)?;
    let desired = desired_agents(app_config, &intent, &current)?;
    let mut steps = build_steps(&context, &current, &desired)?;
    preflight_steps(&context.canonical_path, &mut steps)?;

    let executor = OsPlacementExecutor;
    let completed = match execute_plan(&executor, &context.canonical_path, &steps) {
        Ok(completed) => completed,
        Err((completed, error)) => {
            return fail_with_compensation(db, &executor, &context, &intent, completed, error)
        }
    };

    let logical_changed = current.keys().cloned().collect::<BTreeSet<_>>() != desired;
    let catalog_result = (|| -> AppResult<Vec<SkillRow>> {
        let mut conn = db.connection()?;
        let tx = conn.transaction()?;
        replace_destination_rows(&tx, &context, &desired)?;
        resolve_reconciliation_evidence(&tx, &context)?;
        let catalog = catalog_from_connection(&tx)?;
        tx.commit()?;
        Ok(catalog)
    })();

    match catalog_result {
        Ok(skills) => Ok(ProjectCustomSkillMutationResult {
            changed: logical_changed || !completed.is_empty(),
            skills,
        }),
        Err(error) => fail_with_compensation(db, &executor, &context, &intent, completed, error),
    }
}

fn load_intent_context(
    db: &Database,
    intent: &ProjectCustomSkillIntent,
) -> AppResult<IntentContext> {
    let (skill_id, destination) = match intent {
        ProjectCustomSkillIntent::SetTargetEnabled {
            skill_id,
            destination,
            ..
        }
        | ProjectCustomSkillIntent::SetAgentPlacement {
            skill_id,
            destination,
            ..
        } => (
            required_trimmed(skill_id, "skill id")?.to_string(),
            destination,
        ),
    };

    let canonical_path = {
        let conn = db.connection()?;
        let row = conn
            .query_row(
                "SELECT source_kind, project_id, canonical_path FROM skills WHERE id = ?1",
                [&skill_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| AppError::Validation("skill was not found".to_string()))?;
        if row.0 != SOURCE_KIND_PROJECT_CUSTOM {
            return Err(AppError::Validation(
                "only Project custom Skills accept propagation intents".to_string(),
            ));
        }
        if row.1.is_none() {
            return Err(AppError::Validation(
                "skill has no source Project".to_string(),
            ));
        }
        PathBuf::from(row.2)
    };

    let destination = match destination {
        ProjectCustomSkillDestination::Global => DestinationContext::Global,
        ProjectCustomSkillDestination::Project { project_id } => {
            let project_id = required_trimmed(project_id, "target project id")?.to_string();
            let root = {
                let conn = db.connection()?;
                projects::list_effective_projects(&conn)?
                    .into_iter()
                    .find(|project| project.id == project_id)
                    .map(|project| project.path)
                    .ok_or_else(|| {
                        AppError::Validation(
                            "target project was not found or is not effectively active".to_string(),
                        )
                    })?
            };
            DestinationContext::Project { project_id, root }
        }
    };

    Ok(IntentContext {
        skill_id,
        canonical_path,
        destination,
    })
}

fn current_agents(db: &Database, context: &IntentContext) -> AppResult<BTreeMap<String, String>> {
    let conn = db.connection()?;
    let mut current = BTreeMap::new();
    match &context.destination {
        DestinationContext::Global => {
            let mut stmt = conn.prepare(
                "SELECT agent, target_path FROM skill_distributions WHERE skill_id = ?1 AND role = 'target'",
            )?;
            let rows = stmt.query_map([&context.skill_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            for row in rows {
                let (agent, path) = row?;
                current.insert(agent, path);
            }
        }
        DestinationContext::Project { project_id, .. } => {
            let mut stmt = conn.prepare(
                r#"
                SELECT agent, target_path
                FROM skill_project_distributions
                WHERE skill_id = ?1 AND target_project_id = ?2 AND role = 'target'
                "#,
            )?;
            let rows = stmt.query_map(params![context.skill_id, project_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            for row in rows {
                let (agent, path) = row?;
                current.insert(agent, path);
            }
        }
    }
    Ok(current)
}

fn desired_agents(
    app_config: &AppConfigService,
    intent: &ProjectCustomSkillIntent,
    current: &BTreeMap<String, String>,
) -> AppResult<BTreeSet<String>> {
    let mut desired = current.keys().cloned().collect::<BTreeSet<_>>();
    match intent {
        ProjectCustomSkillIntent::SetTargetEnabled { enabled, .. } => {
            if *enabled {
                if desired.is_empty() {
                    desired.insert(default_entry_agent(app_config)?);
                }
            } else {
                desired.clear();
            }
        }
        ProjectCustomSkillIntent::SetAgentPlacement { agent, enabled, .. } => {
            let name = required_trimmed(agent, "agent")?;
            let surface = agent_by_name(name)
                .ok_or_else(|| AppError::Validation(format!("unknown agent: {name}")))?;
            if surface.skill.is_none() {
                return Err(AppError::Validation(format!(
                    "{} does not support skill placement",
                    surface.name
                )));
            }
            if *enabled {
                desired.insert(surface.name.to_string());
            } else {
                desired.remove(surface.name);
            }
        }
    }
    Ok(desired)
}

fn default_entry_agent(app_config: &AppConfigService) -> AppResult<String> {
    let preferences = app_config.get_agent_display_preferences()?;
    let candidate = preferences
        .default_global_entry_agent
        .unwrap_or_else(|| "Generic Agent".to_string());
    let surface = agent_by_name(&candidate)
        .ok_or_else(|| AppError::Validation(format!("unknown agent: {candidate}")))?;
    if surface.skill.is_none() || preferences.disabled.iter().any(|name| name == surface.name) {
        return Err(AppError::Validation(format!(
            "default entry Agent is not available for Skill placement: {}",
            surface.name
        )));
    }
    Ok(surface.name.to_string())
}

fn target_path(context: &IntentContext, agent_name: &str) -> AppResult<PathBuf> {
    let agent = agent_by_name(agent_name)
        .ok_or_else(|| AppError::Validation(format!("unknown agent: {agent_name}")))?;
    let skill_surface = agent.skill.ok_or_else(|| {
        AppError::Validation(format!("{} does not support skill placement", agent.name))
    })?;
    let dir_name = context
        .canonical_path
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::Validation("skill path has no valid directory name".to_string())
        })?;
    match &context.destination {
        DestinationContext::Global => {
            Ok(paths::resolve_local_path(skill_surface.global_dir)?.join(dir_name))
        }
        DestinationContext::Project { root, .. } => {
            Ok(root.join(skill_surface.project_dir).join(dir_name))
        }
    }
}

fn build_steps(
    context: &IntentContext,
    current: &BTreeMap<String, String>,
    desired: &BTreeSet<String>,
) -> AppResult<Vec<PlacementStep>> {
    let mut steps = Vec::new();
    for agent in agent_capability_surfaces()
        .iter()
        .filter(|agent| agent.skill.is_some())
    {
        if desired.contains(agent.name) {
            let path = target_path(context, agent.name)?;
            steps.push(PlacementStep {
                action: StepAction::Create,
                agent: agent.name.to_string(),
                path: path_to_string(&path, "skill target path")?,
                path_buf: path,
                execute: false,
                missing_parent_dirs: Vec::new(),
            });
        } else if let Some(path) = current.get(agent.name) {
            let path_buf = PathBuf::from(path);
            steps.push(PlacementStep {
                action: StepAction::Remove,
                agent: agent.name.to_string(),
                path: path.clone(),
                path_buf,
                execute: false,
                missing_parent_dirs: Vec::new(),
            });
        }
    }
    Ok(steps)
}

fn preflight_steps(canonical_path: &Path, steps: &mut [PlacementStep]) -> AppResult<()> {
    if !canonical_path.is_dir() {
        return Err(AppError::Validation(format!(
            "skill canonical source does not exist: {}",
            canonical_path.display()
        )));
    }
    for step in steps {
        let metadata = fs::symlink_metadata(&step.path_buf);
        step.execute = match step.action {
            StepAction::Create => {
                if distribution::placement_points_to(&step.path_buf, canonical_path)? {
                    false
                } else {
                    match metadata {
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                            step.missing_parent_dirs = preflight_parent_chain(&step.path_buf)?;
                            true
                        }
                        Err(error) => return Err(error.into()),
                        Ok(_) => {
                            return Err(AppError::Validation(format!(
                                "skill target path already exists with conflicting content: {}",
                                step.path_buf.display()
                            )))
                        }
                    }
                }
            }
            StepAction::Remove => match metadata {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
                Err(error) => return Err(error.into()),
                Ok(_) if distribution::placement_points_to(&step.path_buf, canonical_path)? => true,
                Ok(_) => {
                    return Err(AppError::Validation(format!(
                        "managed Skill placement was replaced and cannot be removed: {}",
                        step.path_buf.display()
                    )))
                }
            },
        };
    }
    Ok(())
}

fn preflight_parent_chain(target: &Path) -> AppResult<Vec<PathBuf>> {
    let mut missing = Vec::new();
    let mut current = target.parent();
    while let Some(parent) = current {
        match fs::metadata(parent) {
            Ok(metadata) if metadata.is_dir() => return Ok(missing),
            Ok(_) => {
                return Err(AppError::Validation(format!(
                    "skill target parent is not a directory: {}",
                    parent.display()
                )))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                missing.push(parent.to_path_buf());
                current = parent.parent();
            }
            Err(error) => return Err(error.into()),
        }
    }
    Ok(missing)
}

fn remove_created_parent_dirs(step: &PlacementStep) {
    for path in &step.missing_parent_dirs {
        match fs::remove_dir(path) {
            Ok(()) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::DirectoryNotEmpty
                ) => {}
            Err(_) => {}
        }
    }
}

trait PlacementExecutor {
    fn execute(&self, canonical_path: &Path, step: &PlacementStep) -> AppResult<()>;
    fn compensate(&self, canonical_path: &Path, step: &PlacementStep) -> AppResult<()>;
}

struct OsPlacementExecutor;

impl PlacementExecutor for OsPlacementExecutor {
    fn execute(&self, canonical_path: &Path, step: &PlacementStep) -> AppResult<()> {
        match step.action {
            StepAction::Create => {
                let result = create_managed_directory_link(canonical_path, &step.path_buf);
                if result.is_err() {
                    remove_created_parent_dirs(step);
                }
                result
            }
            StepAction::Remove => {
                remove_managed_directory_link_if_present(canonical_path, &step.path_buf)
            }
        }
    }

    fn compensate(&self, canonical_path: &Path, step: &PlacementStep) -> AppResult<()> {
        match step.action {
            StepAction::Create => {
                remove_managed_directory_link_if_present(canonical_path, &step.path_buf)?;
                remove_created_parent_dirs(step);
                Ok(())
            }
            StepAction::Remove => create_managed_directory_link(canonical_path, &step.path_buf),
        }
    }
}

fn execute_plan(
    executor: &impl PlacementExecutor,
    canonical_path: &Path,
    steps: &[PlacementStep],
) -> Result<Vec<PlacementStep>, (Vec<PlacementStep>, AppError)> {
    let mut completed = Vec::new();
    for step in steps.iter().filter(|step| step.execute) {
        if let Err(error) = executor.execute(canonical_path, step) {
            return Err((completed, error));
        }
        completed.push(step.clone());
    }
    Ok(completed)
}

fn compensate_completed(
    executor: &impl PlacementExecutor,
    canonical_path: &Path,
    completed: &[PlacementStep],
) -> Vec<FailedCompensation> {
    let mut failed = Vec::new();
    for step in completed.iter().rev() {
        if let Err(error) = executor.compensate(canonical_path, step) {
            failed.push(FailedCompensation {
                step: step.clone(),
                error: error.to_string(),
            });
        }
    }
    failed
}

fn replace_destination_rows(
    conn: &Connection,
    context: &IntentContext,
    desired: &BTreeSet<String>,
) -> AppResult<()> {
    match &context.destination {
        DestinationContext::Global => {
            conn.execute(
                "DELETE FROM skill_distributions WHERE skill_id = ?1",
                [&context.skill_id],
            )?;
            for agent in agent_capability_surfaces()
                .iter()
                .filter(|agent| agent.skill.is_some())
            {
                let enabled = desired.contains(agent.name);
                let path = enabled
                    .then(|| target_path(context, agent.name))
                    .transpose()?
                    .map(|path| path_to_string(&path, "skill target path"))
                    .transpose()?;
                conn.execute(
                    r#"
                    INSERT INTO skill_distributions (skill_id, agent, role, target_path)
                    VALUES (?1, ?2, ?3, ?4)
                    "#,
                    params![
                        context.skill_id,
                        agent.name,
                        if enabled { "target" } else { "none" },
                        path
                    ],
                )?;
            }
        }
        DestinationContext::Project { project_id, .. } => {
            conn.execute(
                "DELETE FROM skill_project_distributions WHERE skill_id = ?1 AND target_project_id = ?2",
                params![context.skill_id, project_id],
            )?;
            for agent in agent_capability_surfaces()
                .iter()
                .filter(|agent| agent.skill.is_some())
            {
                let enabled = desired.contains(agent.name);
                let path = enabled
                    .then(|| target_path(context, agent.name))
                    .transpose()?
                    .map(|path| path_to_string(&path, "skill target path"))
                    .transpose()?;
                conn.execute(
                    r#"
                    INSERT INTO skill_project_distributions (
                        skill_id, target_project_id, agent, role, target_path
                    ) VALUES (?1, ?2, ?3, ?4, ?5)
                    "#,
                    params![
                        context.skill_id,
                        project_id,
                        agent.name,
                        if enabled { "target" } else { "none" },
                        path
                    ],
                )?;
            }
        }
    }
    Ok(())
}

fn resolve_reconciliation_evidence(conn: &Connection, context: &IntentContext) -> AppResult<()> {
    let now = now_epoch_seconds()?;
    conn.execute(
        r#"
        UPDATE skill_propagation_reconciliations
        SET resolved_at = ?4
        WHERE skill_id = ?1
          AND destination_kind = ?2
          AND (target_project_id IS ?3 OR target_project_id = ?3)
          AND resolved_at IS NULL
        "#,
        params![
            context.skill_id,
            context.destination.kind(),
            context.destination.project_id(),
            now
        ],
    )?;
    Ok(())
}

fn fail_with_compensation(
    db: &Database,
    executor: &impl PlacementExecutor,
    context: &IntentContext,
    intent: &ProjectCustomSkillIntent,
    completed: Vec<PlacementStep>,
    original: AppError,
) -> AppResult<ProjectCustomSkillMutationResult> {
    let failed = compensate_completed(executor, &context.canonical_path, &completed);
    if failed.is_empty() {
        return Err(original);
    }

    let evidence_id = Uuid::new_v4().to_string();
    let observed = completed
        .iter()
        .map(|step| {
            let state = match fs::symlink_metadata(&step.path_buf) {
                Ok(_) => "present",
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => "missing",
                Err(_) => "unreadable",
            };
            (step.path.clone(), state)
        })
        .collect::<BTreeMap<_, _>>();
    let persist = persist_evidence(
        db,
        &evidence_id,
        context,
        intent,
        &completed,
        &failed,
        &observed,
    );
    let persist_suffix = persist
        .err()
        .map(|error| format!("; evidence persistence also failed: {error}"))
        .unwrap_or_default();
    Err(AppError::Reconciliation(format!(
        "Project custom Skill propagation requires reconciliation (evidence {evidence_id}); original error: {original}; compensation failures: {}{persist_suffix}",
        failed
            .iter()
            .map(|failure| format!("{}: {}", failure.step.path, failure.error))
            .collect::<Vec<_>>()
            .join(", ")
    )))
}

#[allow(clippy::too_many_arguments)]
fn persist_evidence(
    db: &Database,
    evidence_id: &str,
    context: &IntentContext,
    intent: &ProjectCustomSkillIntent,
    completed: &[PlacementStep],
    failed: &[FailedCompensation],
    observed: &BTreeMap<String, &str>,
) -> AppResult<()> {
    let intent_json = serde_json::to_string(intent)
        .map_err(|error| AppError::Internal(format!("serialize propagation intent: {error}")))?;
    let completed_json = serde_json::to_string(completed)
        .map_err(|error| AppError::Internal(format!("serialize completed steps: {error}")))?;
    let failed_json = serde_json::to_string(failed)
        .map_err(|error| AppError::Internal(format!("serialize failed compensations: {error}")))?;
    let observed_json = serde_json::to_string(observed)
        .map_err(|error| AppError::Internal(format!("serialize observed paths: {error}")))?;
    let now = now_epoch_seconds()?;
    let conn = db.connection()?;
    conn.execute(
        r#"
        INSERT INTO skill_propagation_reconciliations (
            id, skill_id, destination_kind, target_project_id, intent_json,
            completed_steps_json, failed_compensations_json, observed_paths_json,
            created_at, resolved_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL)
        "#,
        params![
            evidence_id,
            context.skill_id,
            context.destination.kind(),
            context.destination.project_id(),
            intent_json,
            completed_json,
            failed_json,
            observed_json,
            now
        ],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    struct ScriptedExecutor {
        forward_calls: Mutex<usize>,
        compensation_calls: Mutex<Vec<String>>,
        fail_forward_at: Option<usize>,
        fail_compensation_for: Option<String>,
    }

    impl PlacementExecutor for ScriptedExecutor {
        fn execute(&self, _canonical_path: &Path, _step: &PlacementStep) -> AppResult<()> {
            let mut calls = self.forward_calls.lock().expect("forward calls");
            *calls += 1;
            if self.fail_forward_at == Some(*calls) {
                return Err(AppError::Io(format!(
                    "scripted forward failure at {}",
                    *calls
                )));
            }
            Ok(())
        }

        fn compensate(&self, _canonical_path: &Path, step: &PlacementStep) -> AppResult<()> {
            self.compensation_calls
                .lock()
                .expect("compensation calls")
                .push(step.agent.clone());
            if self.fail_compensation_for.as_deref() == Some(step.agent.as_str()) {
                return Err(AppError::Io(format!(
                    "scripted compensation failure for {}",
                    step.agent
                )));
            }
            Ok(())
        }
    }

    fn step(agent: &str) -> PlacementStep {
        PlacementStep {
            action: StepAction::Create,
            agent: agent.to_string(),
            path: format!("/{agent}"),
            path_buf: PathBuf::from(format!("/{agent}")),
            execute: true,
            missing_parent_dirs: Vec::new(),
        }
    }

    #[test]
    fn forward_failure_compensates_only_completed_steps_in_reverse_order() {
        let executor = ScriptedExecutor {
            forward_calls: Mutex::new(0),
            compensation_calls: Mutex::new(Vec::new()),
            fail_forward_at: Some(3),
            fail_compensation_for: None,
        };
        let steps = vec![step("Generic Agent"), step("Claude Code"), step("CodeX")];
        let (completed, error) = execute_plan(&executor, Path::new("/source"), &steps)
            .expect_err("third forward step fails");
        assert!(error.to_string().contains("scripted forward failure"));
        assert_eq!(
            completed
                .iter()
                .map(|step| step.agent.as_str())
                .collect::<Vec<_>>(),
            vec!["Generic Agent", "Claude Code"]
        );

        let failed = compensate_completed(&executor, Path::new("/source"), &completed);
        assert!(failed.is_empty());
        assert_eq!(
            *executor.compensation_calls.lock().expect("calls"),
            vec!["Claude Code".to_string(), "Generic Agent".to_string()]
        );
    }

    #[test]
    fn compensation_failure_is_reported_separately() {
        let executor = ScriptedExecutor {
            forward_calls: Mutex::new(0),
            compensation_calls: Mutex::new(Vec::new()),
            fail_forward_at: None,
            fail_compensation_for: Some("Claude Code".to_string()),
        };
        let completed = vec![step("Generic Agent"), step("Claude Code")];
        let failed = compensate_completed(&executor, Path::new("/source"), &completed);
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].step.agent, "Claude Code");
        assert!(failed[0].error.contains("scripted compensation failure"));
    }

    #[test]
    fn compensation_failure_persists_evidence_and_returns_reconciliation_error() {
        let db = Database::open_in_memory().expect("database");
        let executor = ScriptedExecutor {
            forward_calls: Mutex::new(0),
            compensation_calls: Mutex::new(Vec::new()),
            fail_forward_at: None,
            fail_compensation_for: Some("Claude Code".to_string()),
        };
        let context = IntentContext {
            skill_id: "skill-evidence".to_string(),
            canonical_path: PathBuf::from("/source"),
            destination: DestinationContext::Global,
        };
        let intent = ProjectCustomSkillIntent::SetTargetEnabled {
            skill_id: context.skill_id.clone(),
            destination: ProjectCustomSkillDestination::Global,
            enabled: false,
        };
        let error = fail_with_compensation(
            &db,
            &executor,
            &context,
            &intent,
            vec![step("Claude Code")],
            AppError::Io("original".to_string()),
        )
        .expect_err("compensation failure returns reconciliation error");
        assert!(matches!(error, AppError::Reconciliation(_)));
        assert!(error.to_string().contains("evidence"));
        let conn = db.connection().expect("connection");
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM skill_propagation_reconciliations WHERE skill_id = 'skill-evidence'",
                [],
                |row| row.get(0),
            )
            .expect("count evidence");
        assert_eq!(count, 1);
    }
}
