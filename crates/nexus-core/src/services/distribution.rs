use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use rusqlite::{params, Connection};

use crate::{
    database::Database,
    error::AppResult,
    services::agent_capabilities::{agent_capability_surfaces, AgentCapabilitySurface},
    services::paths::path_to_string,
    services::placement::scanned_target_identity,
    services::symlink::is_junction,
};

/// A distributable asset's scanned source, viewed as an Agent Matrix row generator. Skill
/// and Prompt implement this to supply their differences (the target-path rule, the optional
/// "already a target" check); the shared invariants live in `matrix_rows`.
pub trait MatrixSource {
    /// Name of the agent that owns the canonical source.
    fn source_agent(&self) -> &str;
    /// Canonical source path that managed placements must point back to.
    fn canonical_path(&self) -> &Path;
    /// Managed target Placement path for `agent`, or None when the agent has no surface for
    /// this asset (or this scope).
    fn target_path_for(&self, agent: &AgentCapabilitySurface) -> AppResult<Option<PathBuf>>;
    /// Error label used when a target path is not valid UTF-8.
    fn target_path_label(&self) -> &'static str;
    /// Extra "this agent is already a target" signal beyond a live placement. Defaults to
    /// false; Prompt honours a previously-recorded target path that still exists on disk.
    fn is_existing_target(&self, _target_path: &Path) -> AppResult<bool> {
        Ok(false)
    }
}

/// Build the Agent Matrix rows for one asset: exactly one `source` (the owning agent), every
/// other agent `target` (when a managed placement is live) or `none`.
pub fn matrix_rows(
    source: &impl MatrixSource,
) -> AppResult<Vec<(&'static str, String, Option<String>)>> {
    let mut rows = Vec::new();
    for agent in agent_capability_surfaces() {
        if agent.name == source.source_agent() {
            rows.push((agent.name, "source".to_string(), None));
            continue;
        }

        let target_path = source.target_path_for(agent)?;
        let role = if let Some(target_path) = &target_path {
            if placement_points_to(target_path, source.canonical_path())?
                || source.is_existing_target(target_path)?
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
                    .map(|path| path_to_string(path, source.target_path_label()))
                    .transpose()?
            } else {
                None
            },
        ));
    }

    Ok(rows)
}

/// Toggle a single Agent Matrix target: place or remove the on-disk link, then upsert the
/// distribution row, rolling the placement back if the row write fails.
#[allow(clippy::too_many_arguments)]
pub fn write_target(
    db: &Database,
    distribution_table: &str,
    id_column: &str,
    asset_id: &str,
    target_agent: &str,
    enabled: bool,
    canonical_path: &Path,
    target_path: &Path,
    target_path_label: &str,
    place: fn(&Path, &Path) -> AppResult<()>,
    remove: fn(&Path, &Path) -> AppResult<()>,
) -> AppResult<()> {
    let created_placement = if enabled {
        place(canonical_path, target_path)?;
        true
    } else {
        remove(canonical_path, target_path)?;
        false
    };

    let result = (|| -> AppResult<()> {
        let target_path_value = if enabled {
            Some(path_to_string(target_path, target_path_label)?)
        } else {
            None
        };
        let conn = db.connection()?;
        conn.execute(
            &format!(
                r#"
                INSERT INTO {distribution_table} ({id_column}, agent, role, target_path)
                VALUES (?1, ?2, ?3, ?4)
                ON CONFLICT({id_column}, agent) DO UPDATE SET
                    role = excluded.role,
                    target_path = excluded.target_path
                "#
            ),
            params![
                asset_id,
                target_agent,
                if enabled { "target" } else { "none" },
                target_path_value,
            ],
        )?;
        Ok(())
    })();

    if result.is_err() && created_placement {
        let _ = remove(canonical_path, target_path);
    }

    result
}

/// All agents' roles for one asset, defaulting to `none`, overlaid with the stored rows.
pub fn cells(
    conn: &Connection,
    distribution_table: &str,
    id_column: &str,
    asset_id: &str,
) -> rusqlite::Result<BTreeMap<String, String>> {
    let mut cells = empty_cells();
    let mut stmt = conn.prepare(&format!(
        "SELECT agent, role FROM {distribution_table} WHERE {id_column} = ?1"
    ))?;
    let rows = stmt.query_map([asset_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;

    for row in rows {
        let (agent, role) = row?;
        cells.insert(agent, role);
    }

    Ok(cells)
}

/// Every known agent mapped to `none` — the base layer for an Agent Matrix row set.
pub fn empty_cells() -> BTreeMap<String, String> {
    agent_capability_surfaces()
        .iter()
        .map(|agent| (agent.name.to_string(), "none".to_string()))
        .collect()
}

/// Existing project-scoped Skill and Prompt Distribution placements, keyed the same way as
/// Project Symlink inventory scan results. A recorded target is managed only while it still
/// resolves to the recorded canonical source.
pub fn project_managed_target_identities(conn: &Connection) -> AppResult<HashSet<String>> {
    let mut stmt = conn.prepare(
        r#"
        SELECT skill.canonical_path, distribution.target_path
        FROM skills skill
        JOIN skill_distributions distribution ON distribution.skill_id = skill.id
        WHERE skill.scope = 'project'
          AND skill.source_kind = 'agent'
          AND distribution.role = 'target'
          AND distribution.target_path IS NOT NULL

        UNION ALL

        SELECT prompt.canonical_path, distribution.target_path
        FROM prompts prompt
        JOIN prompt_distributions distribution ON distribution.prompt_id = prompt.id
        WHERE prompt.scope = 'project'
          AND distribution.role = 'target'
          AND distribution.target_path IS NOT NULL
        "#,
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut identities = HashSet::new();

    for row in rows {
        let (source, target) = row?;
        let target = Path::new(&target);
        if placement_points_to(target, Path::new(&source))? {
            identities.insert(scanned_target_identity(target)?);
        }
    }

    Ok(identities)
}

/// Whether `target_path` is a managed placement (symlink or junction) resolving to `source_path`.
fn placement_points_to(target_path: &Path, source_path: &Path) -> AppResult<bool> {
    let metadata = match fs::symlink_metadata(target_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    // A managed placement is a symlink (Unix / elevated Windows) or a junction (Windows).
    if !metadata.file_type().is_symlink() && !is_junction(target_path) {
        return Ok(false);
    }

    // Compare canonical paths so both symlink and junction placements resolve to the source.
    let Ok(resolved_target) = target_path.canonicalize() else {
        return Ok(false);
    };
    Ok(resolved_target == source_path.canonicalize()?)
}
