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
    services::symlink::{create_managed_directory_link, remove_managed_directory_link_if_present},
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
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetSkillTargetInput {
    pub skill_id: String,
    pub agent: String,
    pub enabled: bool,
}

#[derive(Clone)]
pub struct SkillService {
    db: Arc<Database>,
}

#[derive(Debug, Clone)]
struct ProjectRoot {
    id: String,
    path: PathBuf,
}

#[derive(Debug, Clone)]
struct SkillSource {
    source_agent: &'static str,
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
            SELECT id, name, scope, project_id, COALESCE(description, ''), canonical_path, disabled
            FROM skills
            ORDER BY scope, project_id IS NULL, project_id, name, canonical_path
            "#,
        )?;
        let rows = stmt.query_map([], |row| skill_from_row(row, &conn))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn scan_skills(&self) -> AppResult<Vec<Skill>> {
        let projects = self.list_project_roots()?;
        let mut sources = Vec::new();

        for agent in agent_capability_surfaces() {
            let Some(skill) = agent.skill else {
                continue;
            };
            if let Some(dir) = expand_home(skill.global_dir) {
                sources.extend(discover_skill_sources(agent, "global", None, None, &dir)?);
            }
        }

        for project in &projects {
            for agent in agent_capability_surfaces() {
                let Some(skill) = agent.skill else {
                    continue;
                };
                let dir = project.path.join(skill.project_dir);
                sources.extend(discover_skill_sources(
                    agent,
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
        self.list_skills()
    }

    pub fn set_skill_target(&self, input: SetSkillTargetInput) -> AppResult<Skill> {
        let skill_id = required_trimmed(&input.skill_id, "skill id")?;
        let target_agent = require_agent(required_trimmed(&input.agent, "agent")?)?;
        let context = self.skill_target_context(skill_id)?;
        let source_agent = require_agent(&context.source_agent)?;
        if source_agent.name == target_agent.name {
            return Err(AppError::Validation(
                "source agent cannot be toggled as a target".to_string(),
            ));
        }

        let target_path = target_path_for_parts(
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
        )?;

        self.get_skill(skill_id)
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
            SELECT id, path
            FROM projects
            WHERE status = 'active'
            ORDER BY sort_index IS NULL, sort_index, created_at, name
            "#,
        )?;
        let rows = stmt.query_map([], |row| {
            let path: String = row.get(1)?;
            Ok(ProjectRoot {
                id: row.get(0)?,
                path: PathBuf::from(path),
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
                            updated_at = ?8
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
                            created_at, updated_at
                        )
                        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)
                        "#,
                        params![
                            id,
                            source.name,
                            source.scope,
                            source.project_id,
                            source.desc,
                            canonical_path,
                            if source.disabled { 1 } else { 0 },
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
            SELECT id, name, scope, project_id, COALESCE(description, ''), canonical_path, disabled
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
            SELECT s.scope, p.path, s.canonical_path, d.agent
            FROM skills s
            LEFT JOIN projects p ON p.id = s.project_id
            JOIN skill_distributions d ON d.skill_id = s.id AND d.role = 'source'
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
                    source_agent: row.get(3)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::Validation("skill source was not found".to_string()))
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
    source_agent: String,
}

fn discover_skill_sources(
    agent: &AgentCapabilitySurface,
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
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            continue;
        }

        let skill_md = path.join("SKILL.md");
        if !skill_md.is_file() {
            continue;
        }

        let canonical_path = path.canonicalize()?;
        let metadata = read_skill_metadata(&canonical_path)?;
        sources.push(SkillSource {
            source_agent: agent.name,
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
    scope: &str,
    project_path: Option<&Path>,
    canonical_path: &Path,
    agent: &AgentCapabilitySurface,
) -> AppResult<Option<PathBuf>> {
    let Some(skill) = agent.skill else {
        return Ok(None);
    };
    let dir_name = skill_dir_name(canonical_path)?;
    if scope == "global" {
        return Ok(expand_home(skill.global_dir).map(|dir| dir.join(dir_name)));
    }

    let Some(project_path) = project_path else {
        return Ok(None);
    };
    Ok(Some(project_path.join(skill.project_dir).join(dir_name)))
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
        path: row.get(5)?,
        disabled: row.get::<_, i64>(6)? != 0,
    })
}

fn expand_home(path: &str) -> Option<PathBuf> {
    if let Some(rest) = path.strip_prefix("~/") {
        return paths::home_dir().map(|home| home.join(rest));
    }

    Some(PathBuf::from(path))
}

fn skill_dir_name(path: &Path) -> AppResult<String> {
    path.file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| AppError::Validation("skill path has no valid directory name".to_string()))
}
