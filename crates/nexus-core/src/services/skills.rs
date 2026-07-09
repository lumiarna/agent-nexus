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
    services::projects,
    services::symlink::{
        create_managed_directory_link, is_junction, remove_managed_directory_link_if_present,
    },
    services::system_open::{open_path, reveal_path},
    services::util::{now_epoch_seconds, require_agent, required_trimmed},
};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    pub desc: String,
    pub path: String,
    pub disabled: bool,
    pub cells: BTreeMap<String, String>,
    /// Canonical source kind: `agent` (owned by a fixed Agent skills dir) or
    /// `project_custom` (from a Project custom_skills_dir, no Agent source).
    pub source_kind: String,
    /// Owning Agent for `source_kind = agent`; `None` for `project_custom`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_agent: Option<String>,
    /// Canonical backend `skills.id` for a projection row. Present only on
    /// incoming target-Project rows, where [`Skill::id`] is a composite display
    /// id (`{skill_id}::project::{target_project_id}`) used as a React key.
    /// Mutations on a projection row must pass this canonical id, not `id`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical_skill_id: Option<String>,
    /// `Some("project")` on an incoming target-Project projection row; `None`
    /// on canonical rows. Distinguishes a foreign Skill row (rendered with a
    /// sourceless Agent Matrix driven by `skill_project_distributions`) from
    /// the source Project custom Skill row.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placement_scope: Option<String>,
    /// Target Project id for an incoming projection row. The row is scoped to
    /// this Project so `ProjectDetailView` / the Skill Project tab group it
    /// under the target Project.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placement_project_id: Option<String>,
    /// Source Project id for an incoming projection row, used by the UI to
    /// render the `Project source` tooltip ("Linked from Project custom source
    /// · <source Project name> · <canonical path>"). Equals the canonical
    /// Skill's `project_id`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_project_id: Option<String>,
}

/// Source kind discriminants stored in `skills.source_kind`.
const SOURCE_KIND_AGENT: &str = "agent";
const SOURCE_KIND_PROJECT_CUSTOM: &str = "project_custom";

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

/// Source-side: propagate a `project_custom` Skill to (or cancel it from) a
/// target Project. Enabling places a managed placement into the target
/// Project's default entry Agent project skills dir; cancelling removes every
/// Agent placement for that target Project.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetProjectSkillProjectInput {
    pub skill_id: String,
    pub target_project_id: String,
    pub default_agent: String,
    pub enabled: bool,
}

/// Target-side: toggle a single Agent placement inside an incoming target
/// Project Skill row (the projection row's Agent Matrix cell).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetProjectSkillTargetInput {
    pub skill_id: String,
    pub target_project_id: String,
    pub agent: String,
    pub enabled: bool,
}

/// One row of the `skill_project_distributions` projection query.
struct ProjectionRow {
    skill_id: String,
    target_project_id: String,
    agent: String,
    role: String,
    name: String,
    desc: String,
    canonical_path: String,
    disabled: bool,
    source_project_id: Option<String>,
}

#[derive(Clone)]
pub struct SkillService {
    db: Arc<Database>,
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

impl SkillService {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    pub fn list_skills(&self) -> AppResult<Vec<Skill>> {
        let conn = self.db.connection()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT
                s.id,
                s.name,
                s.scope,
                s.project_id,
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
        let rows = stmt.query_map([], |row| skill_from_row(row, &conn))?;
        let mut skills = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(AppError::from)?;
        drop(stmt);
        self.append_project_projection_rows(&conn, &mut skills)?;
        Ok(skills)
    }

    /// Append target-Project projection rows for `project_custom` Skills,
    /// including placements back into the source Project. Each group of
    /// `skill_project_distributions` rows for one `(skill_id, target_project_id)`
    /// with at least one live `target` becomes one projection `Skill` row,
    /// scoped to the target Project so the Project detail / Skill Project tab
    /// surface it there. Cells come only from `skill_project_distributions`
    /// (`target` / `none`, no `source`); the canonical `id`/path/source come
    /// from the canonical Skill row.
    fn append_project_projection_rows(
        &self,
        conn: &rusqlite::Connection,
        skills: &mut Vec<Skill>,
    ) -> AppResult<()> {
        let mut stmt = conn.prepare(
            r#"
            SELECT
                spd.skill_id,
                spd.target_project_id,
                spd.agent,
                spd.role,
                s.name,
                COALESCE(s.description, ''),
                s.canonical_path,
                s.disabled,
                s.project_id AS source_project_id
            FROM skill_project_distributions spd
            JOIN skills s ON s.id = spd.skill_id
            WHERE s.source_kind = 'project_custom'
            ORDER BY
                s.name,
                s.canonical_path,
                spd.target_project_id
            "#,
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ProjectionRow {
                skill_id: row.get(0)?,
                target_project_id: row.get(1)?,
                agent: row.get(2)?,
                role: row.get(3)?,
                name: row.get(4)?,
                desc: row.get(5)?,
                canonical_path: row.get(6)?,
                disabled: row.get::<_, i64>(7)? != 0,
                source_project_id: row.get(8)?,
            })
        })?;
        let rows = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(AppError::from)?;
        drop(stmt);

        // Group consecutive rows by (skill_id, target_project_id) — the ORDER BY
        // above keeps them contiguous.
        let mut iter = rows.into_iter().peekable();
        while let Some(first) = iter.next() {
            let mut cells = distribution::empty_cells();
            if first.role == "target" {
                cells.insert(first.agent.clone(), "target".to_string());
            }
            let mut has_target = first.role == "target";
            while iter.peek().is_some_and(|next| {
                next.skill_id == first.skill_id && next.target_project_id == first.target_project_id
            }) {
                let next = iter.next().unwrap();
                if next.role == "target" {
                    cells.insert(next.agent.clone(), "target".to_string());
                    has_target = true;
                } else {
                    cells.insert(next.agent.clone(), "none".to_string());
                }
            }
            if !has_target {
                continue;
            }

            let display_id = format!("{}::project::{}", first.skill_id, first.target_project_id);
            skills.push(Skill {
                id: display_id,
                cells,
                name: first.name,
                scope: "project".to_string(),
                project_id: Some(first.target_project_id.clone()),
                desc: first.desc,
                path: paths::collapse_home(&first.canonical_path),
                disabled: first.disabled,
                source_kind: SOURCE_KIND_PROJECT_CUSTOM.to_string(),
                source_agent: None,
                canonical_skill_id: Some(first.skill_id.clone()),
                placement_scope: Some("project".to_string()),
                placement_project_id: Some(first.target_project_id.clone()),
                source_project_id: first.source_project_id,
            });
        }

        Ok(())
    }

    pub fn scan_skills(&self) -> AppResult<Vec<Skill>> {
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

    pub fn move_skill_source(&self, input: MoveSkillSourceInput) -> AppResult<Skill> {
        let skill_id = required_trimmed(&input.skill_id, "skill id")?;
        let target_agent = require_agent(required_trimmed(&input.agent, "agent")?)?;
        if target_agent.skill.is_none() {
            return Err(AppError::Validation(format!(
                "{} does not support skill placement",
                target_agent.name
            )));
        }

        let context = self.skill_target_context(skill_id)?;
        if context.source_kind != SOURCE_KIND_AGENT {
            return Err(AppError::Validation(
                "only Agent-sourced Skills can move source".to_string(),
            ));
        }

        let source_agent = require_agent(context.source_agent.as_deref().unwrap_or_default())?;
        if source_agent.name == target_agent.name {
            return self.get_skill(skill_id);
        }

        let old_canonical_path = context.canonical_path;
        let new_canonical_path = target_path_for_parts(
            SOURCE_KIND_AGENT,
            &context.scope,
            context.project_path.as_deref(),
            &old_canonical_path,
            target_agent,
        )?
        .ok_or_else(|| AppError::Validation("skill target path cannot be computed".to_string()))?;
        let old_source_target_path = old_canonical_path.clone();
        let target_was_target =
            self.distribution_role(skill_id, target_agent.name)? == Some("target".to_string());

        if target_was_target {
            remove_managed_directory_link_if_present(&old_canonical_path, &new_canonical_path)?;
        }
        let move_result = (|| -> AppResult<()> {
            match fs::symlink_metadata(&new_canonical_path) {
                Ok(_) => {
                    return Err(AppError::Validation(format!(
                        "skill target path already exists: {}",
                        new_canonical_path.display()
                    )));
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
            if let Some(parent) = new_canonical_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::rename(&old_canonical_path, &new_canonical_path)?;
            Ok(())
        })();
        if let Err(error) = move_result {
            if target_was_target {
                let _ = create_managed_directory_link(&old_canonical_path, &new_canonical_path);
            }
            return Err(error);
        }
        if let Err(error) =
            create_managed_directory_link(&new_canonical_path, &old_source_target_path)
        {
            let _ = fs::rename(&new_canonical_path, &old_canonical_path);
            if target_was_target {
                let _ = create_managed_directory_link(&old_canonical_path, &new_canonical_path);
            }
            return Err(error);
        }

        let db_result = self.update_moved_skill_source(
            skill_id,
            source_agent.name,
            target_agent.name,
            &old_source_target_path,
            &new_canonical_path,
        );
        if db_result.is_err() {
            let _ = remove_managed_directory_link_if_present(
                &new_canonical_path,
                &old_source_target_path,
            );
            let _ = fs::rename(&new_canonical_path, &old_canonical_path);
            if target_was_target {
                let _ = create_managed_directory_link(&old_canonical_path, &new_canonical_path);
            }
        }
        db_result?;

        self.get_skill(skill_id)
    }

    pub fn set_skill_target(&self, input: SetSkillTargetInput) -> AppResult<Skill> {
        let skill_id = required_trimmed(&input.skill_id, "skill id")?;
        let target_agent = require_agent(required_trimmed(&input.agent, "agent")?)?;
        let context = self.skill_target_context(skill_id)?;
        // An agent-sourced skill cannot toggle its own source cell. A Project custom
        // skill has no Agent source, so every Agent is a valid Global placement target.
        if context.source_kind == SOURCE_KIND_AGENT {
            let source_agent = require_agent(context.source_agent.as_deref().unwrap_or_default())?;
            if source_agent.name == target_agent.name {
                return Err(AppError::Validation(
                    "source agent cannot be toggled as a target".to_string(),
                ));
            }
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

        self.get_skill(skill_id)
    }

    /// Propagate a `project_custom` Skill to (or cancel it from) a target
    /// Project, including its source Project. Enabling places a managed
    /// directory link at the target Project's default entry Agent project
    /// skills dir; cancelling removes
    /// every Agent placement for that target Project so the incoming row
    /// disappears. Returns the full skill list so the UI can refetch
    /// projection rows.
    pub fn set_project_skill_project(
        &self,
        input: SetProjectSkillProjectInput,
    ) -> AppResult<Vec<Skill>> {
        let skill_id = required_trimmed(&input.skill_id, "skill id")?;
        let target_project_id = required_trimmed(&input.target_project_id, "target project id")?;
        let default_agent = require_agent(required_trimmed(&input.default_agent, "agent")?)?;
        let context = self.project_skill_context(skill_id)?;

        let target_root = self.project_root(target_project_id)?;
        let target_path = project_target_path_for_skill(
            &target_root.path,
            &context.canonical_path,
            default_agent,
        )?;

        if input.enabled {
            distribution::write_target(
                &self.db,
                "skill_project_distributions",
                "skill_id",
                skill_id,
                default_agent.name,
                true,
                &context.canonical_path,
                &target_path,
                "skill target path",
                create_managed_directory_link,
                remove_managed_directory_link_if_present,
                Some(("target_project_id", target_project_id)),
            )?;
        } else {
            self.remove_project_skill_placements(skill_id, target_project_id)?;
        }

        self.list_skills()
    }

    /// Toggle a single Agent placement inside an incoming target Project Skill
    /// row. When the last `target` is removed the projection row naturally
    /// disappears on the next `list_skills` (no group with a live target).
    pub fn set_project_skill_target(
        &self,
        input: SetProjectSkillTargetInput,
    ) -> AppResult<Vec<Skill>> {
        let skill_id = required_trimmed(&input.skill_id, "skill id")?;
        let target_project_id = required_trimmed(&input.target_project_id, "target project id")?;
        let target_agent = require_agent(required_trimmed(&input.agent, "agent")?)?;
        let context = self.project_skill_context(skill_id)?;

        let target_root = self.project_root(target_project_id)?;
        let target_path = project_target_path_for_skill(
            &target_root.path,
            &context.canonical_path,
            target_agent,
        )?;

        distribution::write_target(
            &self.db,
            "skill_project_distributions",
            "skill_id",
            skill_id,
            target_agent.name,
            input.enabled,
            &context.canonical_path,
            &target_path,
            "skill target path",
            create_managed_directory_link,
            remove_managed_directory_link_if_present,
            Some(("target_project_id", target_project_id)),
        )?;

        self.list_skills()
    }

    /// Remove every Agent placement for one (skill, target Project) pair:
    /// drop the on-disk managed links first, then delete the distribution rows.
    /// Used by source-side cancellation.
    fn remove_project_skill_placements(
        &self,
        skill_id: &str,
        target_project_id: &str,
    ) -> AppResult<()> {
        let targets: Vec<(String, String)> = {
            let conn = self.db.connection()?;
            let mut stmt = conn.prepare(
                r#"
                SELECT agent, target_path
                FROM skill_project_distributions
                WHERE skill_id = ?1 AND target_project_id = ?2 AND role = 'target'
                "#,
            )?;
            let rows = stmt.query_map(params![skill_id, target_project_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            rows.collect::<Result<Vec<_>, _>>()?
        };

        let canonical_path = self.skill_canonical_path(skill_id)?;
        for (_agent, target_path) in &targets {
            let target = PathBuf::from(target_path);
            // Removal is best-effort per placement; a missing link is not an error.
            let _ = remove_managed_directory_link_if_present(&canonical_path, &target);
        }

        let conn = self.db.connection()?;
        conn.execute(
            r#"
            DELETE FROM skill_project_distributions
            WHERE skill_id = ?1 AND target_project_id = ?2
            "#,
            params![skill_id, target_project_id],
        )?;
        Ok(())
    }

    /// Context for Project propagation: the canonical Skill must be a
    /// `project_custom` source rooted in a source Project. The target may be
    /// that same source Project or another active Project.
    fn project_skill_context(&self, skill_id: &str) -> AppResult<ProjectSkillContext> {
        let conn = self.db.connection()?;
        let row = conn
            .query_row(
                r#"
                SELECT s.source_kind, s.project_id, s.canonical_path
                FROM skills s
                WHERE s.id = ?1
                "#,
                params![skill_id],
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

        let (source_kind, source_project_id, canonical_path) = row;
        if source_kind != SOURCE_KIND_PROJECT_CUSTOM {
            return Err(AppError::Validation(
                "only Project custom Skills can be propagated to Project targets".to_string(),
            ));
        }
        if source_project_id.is_none() {
            return Err(AppError::Validation(
                "skill has no source Project".to_string(),
            ));
        }

        Ok(ProjectSkillContext {
            canonical_path: PathBuf::from(canonical_path),
        })
    }

    /// Fetch one active Project root by id. Returns a validation error if the
    /// Project does not exist or is not active — cross-Project propagation
    /// only targets active Projects.
    fn project_root(&self, project_id: &str) -> AppResult<ProjectRoot> {
        self.list_project_roots()?
            .into_iter()
            .find(|root| root.id == project_id)
            .ok_or_else(|| {
                AppError::Validation("target project was not found or is not active".to_string())
            })
    }

    pub fn set_skill_disabled(&self, id: String, disabled: bool) -> AppResult<Skill> {
        let id = required_trimmed(&id, "skill id")?;
        let canonical_path = self.skill_canonical_path(id)?;
        let skill_file = canonical_path.join("SKILL.md");
        let original = fs::read_to_string(&skill_file)?;
        let next = set_disable_model_invocation(&original, disabled);
        fs::write(&skill_file, next)?;

        let result = (|| -> AppResult<Skill> {
            let now = now_epoch_seconds()?;
            let conn = self.db.connection()?;
            let changed = conn.execute(
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
            drop(conn);
            self.get_skill(id)
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
        let mut stmt = conn.prepare(
            r#"
            SELECT id, path, custom_skills_dirs
            FROM projects
            WHERE status = 'active'
            ORDER BY sort_index IS NULL, sort_index, created_at, name
            "#,
        )?;
        let rows = stmt.query_map([], |row| {
            let path: String = row.get(1)?;
            let custom_skills_dirs: String = row.get(2)?;
            Ok(ProjectRoot {
                id: row.get(0)?,
                path: PathBuf::from(path),
                custom_dirs: projects::parse_dir_list(&custom_skills_dirs),
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
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

    fn get_skill(&self, id: &str) -> AppResult<Skill> {
        let conn = self.db.connection()?;
        conn.query_row(
            r#"
            SELECT id, name, scope, project_id, COALESCE(description, ''), canonical_path, disabled,
                   source_kind, source_agent
            FROM skills
            WHERE id = ?1
            "#,
            params![id],
            |row| skill_from_row(row, &conn),
        )
        .optional()?
        .ok_or_else(|| AppError::Validation("skill was not found".to_string()))
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

    fn distribution_role(&self, skill_id: &str, agent: &str) -> AppResult<Option<String>> {
        let conn = self.db.connection()?;
        conn.query_row(
            "SELECT role FROM skill_distributions WHERE skill_id = ?1 AND agent = ?2",
            params![skill_id, agent],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(Into::into)
    }

    fn update_moved_skill_source(
        &self,
        skill_id: &str,
        old_source_agent: &str,
        new_source_agent: &str,
        old_source_target_path: &Path,
        new_canonical_path: &Path,
    ) -> AppResult<()> {
        let old_source_target_path = path_to_string(old_source_target_path, "skill target path")?;
        let new_canonical_path = path_to_string(new_canonical_path, "skill path")?;
        let now = now_epoch_seconds()?;
        let mut conn = self.db.connection()?;
        let tx = conn.transaction()?;

        tx.execute(
            r#"
            UPDATE skills
            SET canonical_path = ?2,
                source_agent = ?3,
                updated_at = ?4
            WHERE id = ?1
            "#,
            params![skill_id, new_canonical_path, new_source_agent, now],
        )?;

        tx.execute(
            r#"
            INSERT INTO skill_distributions (skill_id, agent, role, target_path)
            VALUES (?1, ?2, 'target', ?3)
            ON CONFLICT(skill_id, agent) DO UPDATE SET
                role = 'target',
                target_path = excluded.target_path
            "#,
            params![skill_id, old_source_agent, old_source_target_path],
        )?;
        tx.execute(
            r#"
            INSERT INTO skill_distributions (skill_id, agent, role, target_path)
            VALUES (?1, ?2, 'source', NULL)
            ON CONFLICT(skill_id, agent) DO UPDATE SET
                role = 'source',
                target_path = NULL
            "#,
            params![skill_id, new_source_agent],
        )?;

        tx.commit()?;
        Ok(())
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

/// Context for Project propagation of a `project_custom` Skill.
struct ProjectSkillContext {
    canonical_path: PathBuf,
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

/// Target path for a cross-Project placement: the target Project's fixed Agent
/// project skills dir (e.g. `<target_project>/.claude/skills/<skill>`). Never
/// the target Project `custom_skills_dirs`, so a placement can never be
/// mistaken for a new canonical source.
fn project_target_path_for_skill(
    target_project_path: &Path,
    canonical_path: &Path,
    agent: &AgentCapabilitySurface,
) -> AppResult<PathBuf> {
    let Some(skill) = agent.skill else {
        return Err(AppError::Validation(format!(
            "{} does not support skill placement",
            agent.name
        )));
    };
    let dir_name = skill_dir_name(canonical_path)?;
    Ok(target_project_path.join(skill.project_dir).join(dir_name))
}

fn skill_from_row(row: &Row<'_>, conn: &rusqlite::Connection) -> rusqlite::Result<Skill> {
    let id: String = row.get(0)?;
    Ok(Skill {
        cells: distribution::cells(conn, "skill_distributions", "skill_id", &id)?,
        id,
        name: row.get(1)?,
        scope: row.get(2)?,
        project_id: row.get(3)?,
        desc: row.get(4)?,
        path: paths::collapse_home(&row.get::<_, String>(5)?),
        disabled: row.get::<_, i64>(6)? != 0,
        source_kind: row.get(7)?,
        source_agent: row.get(8)?,
        canonical_skill_id: None,
        placement_scope: None,
        placement_project_id: None,
        source_project_id: None,
    })
}

fn skill_dir_name(path: &Path) -> AppResult<String> {
    path.file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| AppError::Validation("skill path has no valid directory name".to_string()))
}
