use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use rusqlite::{params, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    database::Database,
    error::{AppError, AppResult},
    services::agent_capabilities::{agent_capability_surfaces, AgentCapabilitySurface},
    services::app_config::AppConfigService,
    services::distribution::{self, MatrixSource},
    services::paths::{self, path_to_string},
    services::projects,
    services::symlink::{
        create_managed_directory_link, is_junction, remove_managed_directory_link_if_present,
    },
    services::system_open::{open_path, reveal_path},
    services::util::{now_epoch_seconds, require_agent, required_trimmed},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRef {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillSummary {
    pub skill_id: String,
    pub name: String,
    pub desc: String,
    pub path: String,
    pub disabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AgentCellRole {
    Source,
    Target,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PlacementCellRole {
    Target,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum SkillContext {
    Global,
    Project { project: ProjectRef },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ProjectCustomDestinationState {
    Global {
        cells: BTreeMap<String, PlacementCellRole>,
    },
    Project {
        project: ProjectRef,
        cells: BTreeMap<String, PlacementCellRole>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum SkillRow {
    AgentCanonical {
        row_key: String,
        skill: SkillSummary,
        context: SkillContext,
        source_agent: String,
        cells: BTreeMap<String, AgentCellRole>,
    },
    ProjectCustomCanonical {
        row_key: String,
        skill: SkillSummary,
        source_project: ProjectRef,
        destinations: Vec<ProjectCustomDestinationState>,
    },
    ProjectCustomIncoming {
        row_key: String,
        skill: SkillSummary,
        source_project: ProjectRef,
        target_project: ProjectRef,
        cells: BTreeMap<String, PlacementCellRole>,
    },
}

impl SkillRow {
    pub fn row_key(&self) -> &str {
        match self {
            Self::AgentCanonical { row_key, .. }
            | Self::ProjectCustomCanonical { row_key, .. }
            | Self::ProjectCustomIncoming { row_key, .. } => row_key,
        }
    }

    pub fn skill(&self) -> &SkillSummary {
        match self {
            Self::AgentCanonical { skill, .. }
            | Self::ProjectCustomCanonical { skill, .. }
            | Self::ProjectCustomIncoming { skill, .. } => skill,
        }
    }
}

pub(crate) const SOURCE_KIND_AGENT: &str = "agent";
pub(crate) const SOURCE_KIND_PROJECT_CUSTOM: &str = "project_custom";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetSkillTargetInput {
    pub skill_id: String,
    pub agent: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveSkillSourceInput {
    pub skill_id: String,
    pub agent: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ProjectCustomSkillDestination {
    Global,
    Project { project_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ProjectCustomSkillIntent {
    SetTargetEnabled {
        skill_id: String,
        destination: ProjectCustomSkillDestination,
        enabled: bool,
    },
    SetAgentPlacement {
        skill_id: String,
        destination: ProjectCustomSkillDestination,
        agent: String,
        enabled: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCustomSkillMutationResult {
    pub changed: bool,
    pub skills: Vec<SkillRow>,
}

#[derive(Clone)]
pub struct SkillService {
    db: Arc<Database>,
    app_config: AppConfigService,
    mutation_lock: Arc<Mutex<()>>,
}

#[derive(Debug, Clone)]
struct ProjectRoot {
    id: String,
    path: PathBuf,
    custom_dirs: Vec<String>,
}

#[derive(Debug, Clone)]
struct SkillSource {
    /// Owning Agent name, or `""` for a Project custom source (no Agent source cell).
    source_agent: &'static str,
    source_kind: &'static str,
    scope: String,
    project_id: Option<String>,
    project_path: Option<PathBuf>,
    name: String,
    desc: String,
    canonical_path: PathBuf,
    disabled: bool,
}

impl MatrixSource for SkillSource {
    fn source_agent(&self) -> &str {
        self.source_agent
    }

    fn canonical_path(&self) -> &Path {
        &self.canonical_path
    }

    fn target_path_for(&self, agent: &AgentCapabilitySurface) -> AppResult<Option<PathBuf>> {
        target_path_for_parts(
            self.source_kind,
            &self.scope,
            self.project_path.as_deref(),
            &self.canonical_path,
            agent,
        )
    }

    fn target_path_label(&self) -> &'static str {
        "skill target path"
    }
}

#[derive(Debug, Clone)]
struct SkillMetadata {
    name: String,
    desc: String,
    disabled: bool,
}

impl distribution::SourceRelocationAdapter for SkillService {
    type Metadata = ();

    fn database(&self) -> &Database {
        &self.db
    }

    fn plan_relocation(
        &self,
        asset_id: &str,
        target_agent: &'static AgentCapabilitySurface,
    ) -> AppResult<distribution::RelocationPlan<Self::Metadata>> {
        if target_agent.skill.is_none() {
            return Err(AppError::Validation(format!(
                "{} does not support skill placement",
                target_agent.name
            )));
        }
        let context = self.skill_target_context(asset_id)?;
        if context.source_kind != SOURCE_KIND_AGENT {
            return Err(AppError::Validation(
                "only Agent-sourced Skills can move source".to_string(),
            ));
        }
        let source_agent = require_agent(context.source_agent.as_deref().unwrap_or_default())?;
        if source_agent.name == target_agent.name {
            return Ok(distribution::RelocationPlan::Unchanged);
        }
        let new_canonical_path = target_path_for_parts(
            SOURCE_KIND_AGENT,
            &context.scope,
            context.project_path.as_deref(),
            &context.canonical_path,
            target_agent,
        )?
        .ok_or_else(|| AppError::Validation("skill target path cannot be computed".to_string()))?;

        Ok(distribution::RelocationPlan::Move(
            distribution::PreparedSourceMove {
                asset_id: asset_id.to_string(),
                storage: distribution::DistributionStorage::Skill,
                old_source_agent: source_agent.name,
                new_source_agent: target_agent.name,
                old_canonical_path: context.canonical_path,
                new_canonical_path,
                placement_kind: distribution::PlacementKind::Directory,
                metadata: (),
            },
        ))
    }

    fn persist_asset_move(
        &self,
        tx: &Transaction<'_>,
        movement: &distribution::PreparedSourceMove<Self::Metadata>,
        now: i64,
    ) -> AppResult<()> {
        let new_canonical_path = path_to_string(&movement.new_canonical_path, "skill path")?;
        let changed = tx.execute(
            r#"
            UPDATE skills
            SET canonical_path = ?2,
                source_agent = ?3,
                updated_at = ?4
            WHERE id = ?1
            "#,
            params![
                movement.asset_id,
                new_canonical_path,
                movement.new_source_agent,
                now
            ],
        )?;
        if changed == 0 {
            return Err(AppError::Validation("skill was not found".to_string()));
        }
        Ok(())
    }
}

impl SkillService {
    pub fn new(db: Arc<Database>, app_config: AppConfigService) -> Self {
        Self {
            db,
            app_config,
            mutation_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn list_skills(&self) -> AppResult<Vec<SkillRow>> {
        let conn = self.db.connection()?;
        super::project_custom_skill_propagation::catalog_from_connection(&conn)
    }

    pub fn scan_skills(&self) -> AppResult<Vec<SkillRow>> {
        let _guard = self.lock_mutations()?;
        let projects = self.list_project_roots()?;
        let mut sources = Vec::new();

        for agent in agent_capability_surfaces() {
            let Some(skill) = agent.skill else {
                continue;
            };
            let dir = paths::resolve_local_path(skill.global_dir)?;
            sources.extend(discover_skill_sources(
                agent.name,
                SOURCE_KIND_AGENT,
                "global",
                None,
                None,
                &dir,
            )?);
        }

        for project in &projects {
            for agent in agent_capability_surfaces() {
                let Some(skill) = agent.skill else {
                    continue;
                };
                let dir = project.path.join(skill.project_dir);
                sources.extend(discover_skill_sources(
                    agent.name,
                    SOURCE_KIND_AGENT,
                    "project",
                    Some(project.id.clone()),
                    Some(project.path.clone()),
                    &dir,
                )?);
            }

            // Project custom skills dirs: extra Project custom sources with no Agent
            // owner. They are scanned in addition to the fixed Agent dirs above.
            for custom_dir in &project.custom_dirs {
                let dir = resolve_custom_dir(&project.path, custom_dir)?;
                sources.extend(discover_skill_sources(
                    "",
                    SOURCE_KIND_PROJECT_CUSTOM,
                    "project",
                    Some(project.id.clone()),
                    Some(project.path.clone()),
                    &dir,
                )?);
            }
        }

        sources.sort_by(|left, right| {
            left.scope
                .cmp(&right.scope)
                .then_with(|| left.project_id.cmp(&right.project_id))
                .then_with(|| left.name.cmp(&right.name))
                .then_with(|| left.canonical_path.cmp(&right.canonical_path))
        });

        self.replace_scanned_sources(sources)?;
        self.reconcile_project_distributions()?;
        self.list_skills()
    }

    /// After a scan, drop `skill_project_distributions` target rows whose
    /// managed placement no longer resolves to the canonical source (the link
    /// was removed out-of-band, or the target path now holds a real directory).
    /// Canonical sources themselves are never created from these placements —
    /// `discover_skill_sources` skips symlinks/junctions — so this only
    /// reconciles link health, mirroring the Global placement fallback.
    fn reconcile_project_distributions(&self) -> AppResult<()> {
        let conn = self.db.connection()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT spd.skill_id, spd.target_project_id, spd.agent, spd.target_path,
                   s.canonical_path
            FROM skill_project_distributions spd
            JOIN skills s ON s.id = spd.skill_id
            WHERE spd.role = 'target' AND spd.target_path IS NOT NULL
            "#,
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;
        let broken: Vec<(String, String, String)> = rows
            .filter_map(|row| {
                let (skill_id, target_project_id, agent, target_path, canonical_path) = match row {
                    Ok(values) => values,
                    Err(error) => return Some(Err(error)),
                };
                match distribution::placement_points_to(
                    Path::new(&target_path),
                    Path::new(&canonical_path),
                ) {
                    Ok(true) => None,
                    Ok(false) => Some(Ok((skill_id, target_project_id, agent))),
                    Err(_) => None,
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        drop(stmt);

        for (skill_id, target_project_id, agent) in broken {
            conn.execute(
                r#"
                DELETE FROM skill_project_distributions
                WHERE skill_id = ?1 AND target_project_id = ?2 AND agent = ?3
                "#,
                params![skill_id, target_project_id, agent],
            )?;
        }

        Ok(())
    }

    pub fn move_skill_source(&self, input: MoveSkillSourceInput) -> AppResult<Vec<SkillRow>> {
        let _guard = self.lock_mutations()?;
        let skill_id = required_trimmed(&input.skill_id, "skill id")?;
        let target_agent = require_agent(required_trimmed(&input.agent, "agent")?)?;
        distribution::relocate_source(self, skill_id, target_agent)?;
        self.list_skills()
    }

    pub fn set_skill_target(&self, input: SetSkillTargetInput) -> AppResult<Vec<SkillRow>> {
        let _guard = self.lock_mutations()?;
        let skill_id = required_trimmed(&input.skill_id, "skill id")?;
        let target_agent = require_agent(required_trimmed(&input.agent, "agent")?)?;
        let context = self.skill_target_context(skill_id)?;
        if context.source_kind != SOURCE_KIND_AGENT {
            return Err(AppError::Validation(
                "Project custom Skills require a propagation intent".to_string(),
            ));
        }
        let source_agent = require_agent(context.source_agent.as_deref().unwrap_or_default())?;
        if source_agent.name == target_agent.name {
            return Err(AppError::Validation(
                "source agent cannot be toggled as a target".to_string(),
            ));
        }

        let target_path = target_path_for_parts(
            &context.source_kind,
            &context.scope,
            context.project_path.as_deref(),
            &context.canonical_path,
            target_agent,
        )?
        .ok_or_else(|| AppError::Validation("skill target path cannot be computed".to_string()))?;

        distribution::write_target(
            &self.db,
            "skill_distributions",
            "skill_id",
            skill_id,
            target_agent.name,
            input.enabled,
            &context.canonical_path,
            &target_path,
            "skill target path",
            create_managed_directory_link,
            remove_managed_directory_link_if_present,
            None,
        )?;

        self.list_skills()
    }

    pub fn apply_project_custom_skill_intent(
        &self,
        intent: ProjectCustomSkillIntent,
    ) -> AppResult<ProjectCustomSkillMutationResult> {
        let _guard = self.lock_mutations()?;
        super::project_custom_skill_propagation::apply_intent(&self.db, &self.app_config, intent)
    }

    pub fn set_skill_disabled(&self, id: String, disabled: bool) -> AppResult<Vec<SkillRow>> {
        let _guard = self.lock_mutations()?;
        let id = required_trimmed(&id, "skill id")?;
        let canonical_path = self.skill_canonical_path(id)?;
        let skill_file = canonical_path.join("SKILL.md");
        let original = fs::read_to_string(&skill_file)?;
        let next = set_disable_model_invocation(&original, disabled);
        fs::write(&skill_file, next)?;

        let result = (|| -> AppResult<Vec<SkillRow>> {
            let now = now_epoch_seconds()?;
            let mut conn = self.db.connection()?;
            let tx = conn.transaction()?;
            let changed = tx.execute(
                r#"
                UPDATE skills
                SET disabled = ?2,
                    updated_at = ?3
                WHERE id = ?1
                "#,
                params![id, if disabled { 1 } else { 0 }, now],
            )?;
            if changed == 0 {
                return Err(AppError::Validation("skill was not found".to_string()));
            }
            let catalog = super::project_custom_skill_propagation::catalog_from_connection(&tx)?;
            tx.commit()?;
            Ok(catalog)
        })();

        if result.is_err() {
            let _ = fs::write(skill_file, original);
        }

        result
    }

    pub fn open_skill_source(&self, id: String) -> AppResult<()> {
        let id = required_trimmed(&id, "skill id")?;
        let canonical_path = self.skill_canonical_path(id)?;
        open_path(&canonical_path.join("SKILL.md"))
    }

    pub fn reveal_skill_path(&self, id: String) -> AppResult<()> {
        let id = required_trimmed(&id, "skill id")?;
        let canonical_path = self.skill_canonical_path(id)?;
        reveal_path(&canonical_path)
    }

    fn list_project_roots(&self) -> AppResult<Vec<ProjectRoot>> {
        let conn = self.db.connection()?;
        let effective = projects::list_effective_projects(&conn)?;
        let mut roots = Vec::with_capacity(effective.len());
        for project in effective {
            let custom_skills_dirs = conn.query_row(
                "SELECT custom_skills_dirs FROM projects WHERE id = ?1",
                [&project.id],
                |row| row.get::<_, String>(0),
            )?;
            roots.push(ProjectRoot {
                id: project.id,
                path: project.path,
                custom_dirs: projects::parse_dir_list(&custom_skills_dirs),
            });
        }
        Ok(roots)
    }

    fn lock_mutations(&self) -> AppResult<std::sync::MutexGuard<'_, ()>> {
        self.mutation_lock
            .lock()
            .map_err(|_| AppError::Internal("Skill mutation lock poisoned".to_string()))
    }

    fn replace_scanned_sources(&self, sources: Vec<SkillSource>) -> AppResult<()> {
        let now = now_epoch_seconds()?;
        let mut scanned_paths = BTreeSet::new();
        let mut conn = self.db.connection()?;
        let tx = conn.transaction()?;

        for source in sources {
            let canonical_path = path_to_string(&source.canonical_path, "skill path")?;
            if !scanned_paths.insert(canonical_path.clone()) {
                continue;
            }

            let source_agent: Option<&str> = if source.source_kind == SOURCE_KIND_AGENT {
                Some(source.source_agent)
            } else {
                None
            };
            let existing_id = tx
                .query_row(
                    "SELECT id FROM skills WHERE canonical_path = ?1",
                    params![canonical_path],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            let skill_id = match existing_id {
                Some(id) => {
                    tx.execute(
                        r#"
                        UPDATE skills
                        SET name = ?2,
                            scope = ?3,
                            project_id = ?4,
                            description = ?5,
                            canonical_path = ?6,
                            disabled = ?7,
                            source_kind = ?8,
                            source_agent = ?9,
                            updated_at = ?10
                        WHERE id = ?1
                        "#,
                        params![
                            id,
                            source.name,
                            source.scope,
                            source.project_id,
                            source.desc,
                            canonical_path,
                            if source.disabled { 1 } else { 0 },
                            source.source_kind,
                            source_agent,
                            now,
                        ],
                    )?;
                    id
                }
                None => {
                    let id = Uuid::new_v4().to_string();
                    tx.execute(
                        r#"
                        INSERT INTO skills (
                            id, name, scope, project_id, description, canonical_path, disabled,
                            source_kind, source_agent, created_at, updated_at
                        )
                        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)
                        "#,
                        params![
                            id,
                            source.name,
                            source.scope,
                            source.project_id,
                            source.desc,
                            canonical_path,
                            if source.disabled { 1 } else { 0 },
                            source.source_kind,
                            source_agent,
                            now,
                        ],
                    )?;
                    id
                }
            };

            tx.execute(
                "DELETE FROM skill_distributions WHERE skill_id = ?1",
                params![skill_id],
            )?;
            for (agent, role, target_path) in distribution::matrix_rows(&source)? {
                tx.execute(
                    r#"
                    INSERT INTO skill_distributions (skill_id, agent, role, target_path)
                    VALUES (?1, ?2, ?3, ?4)
                    "#,
                    params![skill_id, agent, role, target_path],
                )?;
            }
        }

        let mut stmt = tx.prepare("SELECT id, canonical_path FROM skills")?;
        let existing_rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let existing = existing_rows.collect::<Result<Vec<_>, _>>()?;
        drop(stmt);

        for (id, canonical_path) in existing {
            if !scanned_paths.contains(&canonical_path) {
                tx.execute("DELETE FROM skills WHERE id = ?1", params![id])?;
            }
        }

        tx.commit()?;
        Ok(())
    }

    fn skill_target_context(&self, id: &str) -> AppResult<SkillTargetContext> {
        let conn = self.db.connection()?;
        conn.query_row(
            r#"
            SELECT s.scope, p.path, s.canonical_path, s.source_kind, s.source_agent
            FROM skills s
            LEFT JOIN projects p ON p.id = s.project_id
            WHERE s.id = ?1
            "#,
            params![id],
            |row| {
                let project_path: Option<String> = row.get(1)?;
                let canonical_path: String = row.get(2)?;
                Ok(SkillTargetContext {
                    scope: row.get(0)?,
                    project_path: project_path.map(PathBuf::from),
                    canonical_path: PathBuf::from(canonical_path),
                    source_kind: row.get(3)?,
                    source_agent: row.get(4)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::Validation("skill was not found".to_string()))
    }

    fn skill_canonical_path(&self, id: &str) -> AppResult<PathBuf> {
        let conn = self.db.connection()?;
        conn.query_row(
            "SELECT canonical_path FROM skills WHERE id = ?1",
            params![id],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .map(PathBuf::from)
        .ok_or_else(|| AppError::Validation("skill was not found".to_string()))
    }
}

#[derive(Debug, Clone)]
struct SkillTargetContext {
    scope: String,
    project_path: Option<PathBuf>,
    canonical_path: PathBuf,
    source_kind: String,
    source_agent: Option<String>,
}

fn discover_skill_sources(
    source_agent: &'static str,
    source_kind: &'static str,
    scope: &str,
    project_id: Option<String>,
    project_path: Option<PathBuf>,
    skills_dir: &Path,
) -> AppResult<Vec<SkillSource>> {
    if !skills_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut sources = Vec::new();
    for entry in fs::read_dir(skills_dir)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() || is_junction(&path) || !metadata.is_dir() {
            continue;
        }

        let skill_md = path.join("SKILL.md");
        if !skill_md.is_file() {
            continue;
        }

        let canonical_path = path.canonicalize()?;
        let metadata = read_skill_metadata(&canonical_path)?;
        sources.push(SkillSource {
            source_agent,
            source_kind,
            scope: scope.to_string(),
            project_id: project_id.clone(),
            project_path: project_path.clone(),
            name: metadata.name,
            desc: metadata.desc,
            canonical_path,
            disabled: metadata.disabled,
        });
    }

    Ok(sources)
}

/// Resolve a Project custom skills dir against the Project root. Absolute paths and
/// `~`-prefixed paths are used as-is; relative paths join the Project root.
fn resolve_custom_dir(project_root: &Path, dir: &str) -> AppResult<PathBuf> {
    let trimmed = dir.trim();
    let resolved = paths::resolve_local_path(trimmed)?;
    if resolved.is_absolute() {
        return Ok(resolved);
    }
    Ok(project_root.join(trimmed))
}

fn read_skill_metadata(skill_dir: &Path) -> AppResult<SkillMetadata> {
    let contents = fs::read_to_string(skill_dir.join("SKILL.md"))?;
    let fields = parse_frontmatter_fields(&contents);
    let fallback_name = skill_dir_name(skill_dir)?;

    Ok(SkillMetadata {
        name: fields
            .get("name")
            .filter(|value| !value.is_empty())
            .cloned()
            .unwrap_or(fallback_name),
        desc: fields.get("description").cloned().unwrap_or_default(),
        disabled: fields
            .get("disable-model-invocation")
            .map(|value| value.eq_ignore_ascii_case("true"))
            .unwrap_or(false),
    })
}

fn parse_frontmatter_fields(contents: &str) -> BTreeMap<String, String> {
    let mut fields = BTreeMap::new();
    let mut lines = contents.lines();
    if lines.next().map(str::trim) != Some("---") {
        return fields;
    }

    for line in lines {
        if line.trim() == "---" {
            break;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        fields.insert(key.trim().to_string(), unquote(value.trim()));
    }

    fields
}

fn unquote(value: &str) -> String {
    let quoted = (value.starts_with('"') && value.ends_with('"'))
        || (value.starts_with('\'') && value.ends_with('\''));
    if quoted && value.len() >= 2 {
        value[1..value.len() - 1].to_string()
    } else {
        value.to_string()
    }
}

fn set_disable_model_invocation(contents: &str, disabled: bool) -> String {
    let field = format!("disable-model-invocation: {disabled}");
    let had_trailing_newline = contents.ends_with('\n');
    let mut lines = contents.lines().map(ToOwned::to_owned).collect::<Vec<_>>();

    if lines.first().map(|line| line.trim()) == Some("---") {
        if let Some(end) = lines
            .iter()
            .enumerate()
            .skip(1)
            .find_map(|(index, line)| (line.trim() == "---").then_some(index))
        {
            let mut updated = false;
            for line in lines.iter_mut().take(end).skip(1) {
                if line.trim_start().starts_with("disable-model-invocation:") {
                    *line = field.clone();
                    updated = true;
                    break;
                }
            }
            if !updated {
                lines.insert(end, field);
            }

            let mut output = lines.join("\n");
            if had_trailing_newline {
                output.push('\n');
            }
            return output;
        }
    }

    format!("---\n{field}\n---\n\n{contents}")
}

fn target_path_for_parts(
    source_kind: &str,
    scope: &str,
    project_path: Option<&Path>,
    canonical_path: &Path,
    agent: &AgentCapabilitySurface,
) -> AppResult<Option<PathBuf>> {
    let Some(skill) = agent.skill else {
        return Ok(None);
    };
    let dir_name = skill_dir_name(canonical_path)?;
    // A Project custom skill propagates to each Agent's *global* skills dir as a
    // managed Global placement, even though its scope stays `project`.
    if scope == "global" || source_kind == SOURCE_KIND_PROJECT_CUSTOM {
        return Ok(Some(
            paths::resolve_local_path(skill.global_dir)?.join(dir_name),
        ));
    }

    let Some(project_path) = project_path else {
        return Ok(None);
    };
    Ok(Some(project_path.join(skill.project_dir).join(dir_name)))
}

fn skill_dir_name(path: &Path) -> AppResult<String> {
    path.file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| AppError::Validation("skill path has no valid directory name".to_string()))
}
