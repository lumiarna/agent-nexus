use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use rusqlite::{params, Connection, OptionalExtension, Transaction};

use crate::{
    database::Database,
    error::{AppError, AppResult},
    services::agent_capabilities::{agent_capability_surfaces, AgentCapabilitySurface},
    services::paths::path_to_string,
    services::placement::scanned_target_identity,
    services::symlink::{
        create_managed_directory_link, create_managed_file_link, is_junction,
        managed_directory_placement_points_to, managed_file_placement_points_to,
        remove_managed_directory_link_if_present, remove_managed_file_link_if_present,
    },
    services::util::now_epoch_seconds,
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
///
/// `extra_key` optionally adds a second natural-key column (name, value) to
/// both the INSERT column list and the ON CONFLICT target. `skill_project_distributions`
/// uses it for `target_project_id` so the `(skill_id, target_project_id, agent)`
/// primary key matches; the legacy single-key tables pass `None`.
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
    extra_key: Option<(&str, &str)>,
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
        match extra_key {
            None => {
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
            }
            Some((extra_col, extra_val)) => {
                conn.execute(
                    &format!(
                        r#"
                        INSERT INTO {distribution_table} ({id_column}, {extra_col}, agent, role, target_path)
                        VALUES (?1, ?2, ?3, ?4, ?5)
                        ON CONFLICT({id_column}, {extra_col}, agent) DO UPDATE SET
                            role = excluded.role,
                            target_path = excluded.target_path
                        "#
                    ),
                    params![
                        asset_id,
                        extra_val,
                        target_agent,
                        if enabled { "target" } else { "none" },
                        target_path_value,
                    ],
                )?;
            }
        }
        Ok(())
    })();

    if result.is_err() && created_placement {
        let _ = remove(canonical_path, target_path);
    }

    result
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DistributionStorage {
    Skill,
    Prompt,
}

impl DistributionStorage {
    fn table(self) -> &'static str {
        match self {
            Self::Skill => "skill_distributions",
            Self::Prompt => "prompt_distributions",
        }
    }

    fn id_column(self) -> &'static str {
        match self {
            Self::Skill => "skill_id",
            Self::Prompt => "prompt_id",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PlacementKind {
    Directory,
    File,
}

pub(crate) enum RelocationPlan<M> {
    Unchanged,
    Move(PreparedSourceMove<M>),
}

pub(crate) struct PreparedSourceMove<M> {
    pub(crate) asset_id: String,
    pub(crate) storage: DistributionStorage,
    pub(crate) old_source_agent: &'static str,
    pub(crate) new_source_agent: &'static str,
    pub(crate) old_canonical_path: PathBuf,
    pub(crate) new_canonical_path: PathBuf,
    pub(crate) placement_kind: PlacementKind,
    pub(crate) metadata: M,
}

pub(crate) trait SourceRelocationAdapter {
    type Metadata;

    fn database(&self) -> &Database;

    /// Read, validate, and calculate asset-specific relocation facts. This must not mutate
    /// either the filesystem or database.
    fn plan_relocation(
        &self,
        asset_id: &str,
        target_agent: &'static AgentCapabilitySurface,
    ) -> AppResult<RelocationPlan<Self::Metadata>>;

    /// Persist only asset-specific fields inside Distribution's transaction.
    fn persist_asset_move(
        &self,
        tx: &Transaction<'_>,
        movement: &PreparedSourceMove<Self::Metadata>,
        now: i64,
    ) -> AppResult<()>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RelocationOutcome {
    Unchanged,
    Moved,
}

pub(crate) fn relocate_source<A: SourceRelocationAdapter>(
    adapter: &A,
    asset_id: &str,
    target_agent: &'static AgentCapabilitySurface,
) -> AppResult<RelocationOutcome> {
    relocate_source_with_runtime(
        adapter,
        asset_id,
        target_agent,
        &FilesystemRelocationRuntime,
    )
}

trait RelocationRuntime {
    fn create_parents(&self, target: &Path) -> AppResult<Vec<PathBuf>>;
    fn rename(&self, source: &Path, target: &Path) -> AppResult<()>;
    fn place(&self, kind: PlacementKind, source: &Path, target: &Path) -> AppResult<()>;
    fn remove(&self, kind: PlacementKind, source: &Path, target: &Path) -> AppResult<()>;
}

struct FilesystemRelocationRuntime;

impl RelocationRuntime for FilesystemRelocationRuntime {
    fn create_parents(&self, target: &Path) -> AppResult<Vec<PathBuf>> {
        let Some(parent) = target.parent() else {
            return Ok(Vec::new());
        };
        let mut missing = Vec::new();
        let mut cursor = parent;
        while !cursor.exists() {
            missing.push(cursor.to_path_buf());
            let Some(next) = cursor.parent() else {
                break;
            };
            cursor = next;
        }

        let mut created = Vec::new();
        for path in missing.iter().rev() {
            match fs::create_dir(path) {
                Ok(()) => created.push(path.clone()),
                Err(error)
                    if error.kind() == std::io::ErrorKind::AlreadyExists && path.is_dir() => {}
                Err(error) => {
                    let mut cleanup_failures = Vec::new();
                    for created_path in created.iter().rev() {
                        if let Err(cleanup_error) = fs::remove_dir(created_path) {
                            cleanup_failures.push(format!(
                                "{}: {}",
                                created_path.display(),
                                cleanup_error
                            ));
                        }
                    }
                    if cleanup_failures.is_empty() {
                        return Err(error.into());
                    }
                    return Err(AppError::Reconciliation(format!(
                        "creating relocation parent {} failed: {}; partial parent cleanup failures: {}",
                        path.display(),
                        error,
                        cleanup_failures.join("; ")
                    )));
                }
            }
        }
        created.reverse();
        Ok(created)
    }

    fn rename(&self, source: &Path, target: &Path) -> AppResult<()> {
        fs::rename(source, target).map_err(Into::into)
    }

    fn place(&self, kind: PlacementKind, source: &Path, target: &Path) -> AppResult<()> {
        match kind {
            PlacementKind::Directory => create_managed_directory_link(source, target),
            PlacementKind::File => create_managed_file_link(source, target),
        }
    }

    fn remove(&self, kind: PlacementKind, source: &Path, target: &Path) -> AppResult<()> {
        match kind {
            PlacementKind::Directory => remove_managed_directory_link_if_present(source, target),
            PlacementKind::File => remove_managed_file_link_if_present(source, target),
        }
    }
}

#[derive(Debug)]
enum UndoStep {
    RestoreRemovedTarget {
        kind: PlacementKind,
        source: PathBuf,
        target: PathBuf,
    },
    RemoveCreatedParents {
        paths: Vec<PathBuf>,
    },
    RenameCanonicalBack {
        from: PathBuf,
        to: PathBuf,
    },
    RemoveOldSourcePlacement {
        kind: PlacementKind,
        source: PathBuf,
        target: PathBuf,
    },
}

impl UndoStep {
    fn label(&self) -> &'static str {
        match self {
            Self::RestoreRemovedTarget { .. } => "restore-removed-target",
            Self::RemoveCreatedParents { .. } => "remove-created-parents",
            Self::RenameCanonicalBack { .. } => "rename-canonical-back",
            Self::RemoveOldSourcePlacement { .. } => "remove-old-source-placement",
        }
    }

    fn paths(&self) -> String {
        match self {
            Self::RestoreRemovedTarget { source, target, .. }
            | Self::RenameCanonicalBack {
                from: source,
                to: target,
            }
            | Self::RemoveOldSourcePlacement { source, target, .. } => {
                format!("{} -> {}", source.display(), target.display())
            }
            Self::RemoveCreatedParents { paths } => paths
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", "),
        }
    }

    fn execute(&self, runtime: &impl RelocationRuntime) -> AppResult<()> {
        match self {
            Self::RestoreRemovedTarget {
                kind,
                source,
                target,
            } => runtime.place(*kind, source, target),
            Self::RemoveCreatedParents { paths } => {
                let mut failures = Vec::new();
                for path in paths {
                    match fs::remove_dir(path) {
                        Ok(()) => {}
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                        Err(error) => failures.push(format!("{}: {}", path.display(), error)),
                    }
                }
                if failures.is_empty() {
                    Ok(())
                } else {
                    Err(AppError::Io(format!(
                        "failed to remove relocation parent directories: {}",
                        failures.join("; ")
                    )))
                }
            }
            Self::RenameCanonicalBack { from, to } => runtime.rename(from, to),
            Self::RemoveOldSourcePlacement {
                kind,
                source,
                target,
            } => runtime.remove(*kind, source, target),
        }
    }
}

enum DestinationState {
    Vacant,
    ManagedPlacement,
}

fn relocate_source_with_runtime<A: SourceRelocationAdapter>(
    adapter: &A,
    asset_id: &str,
    target_agent: &'static AgentCapabilitySurface,
    runtime: &impl RelocationRuntime,
) -> AppResult<RelocationOutcome> {
    let movement = match adapter.plan_relocation(asset_id, target_agent)? {
        RelocationPlan::Unchanged => return Ok(RelocationOutcome::Unchanged),
        RelocationPlan::Move(movement) => movement,
    };

    let destination = preflight_relocation(adapter.database(), &movement)?;
    let mut journal = Vec::new();

    if matches!(destination, DestinationState::ManagedPlacement) {
        runtime.remove(
            movement.placement_kind,
            &movement.old_canonical_path,
            &movement.new_canonical_path,
        )?;
        journal.push(UndoStep::RestoreRemovedTarget {
            kind: movement.placement_kind,
            source: movement.old_canonical_path.clone(),
            target: movement.new_canonical_path.clone(),
        });
    }

    let created_parents = match runtime.create_parents(&movement.new_canonical_path) {
        Ok(paths) => paths,
        Err(error) => {
            return fail_with_compensation("create-target-parents", error, journal, runtime)
        }
    };
    if !created_parents.is_empty() {
        journal.push(UndoStep::RemoveCreatedParents {
            paths: created_parents,
        });
    }

    if let Err(error) = runtime.rename(&movement.old_canonical_path, &movement.new_canonical_path) {
        return fail_with_compensation("rename-canonical", error, journal, runtime);
    }
    journal.push(UndoStep::RenameCanonicalBack {
        from: movement.new_canonical_path.clone(),
        to: movement.old_canonical_path.clone(),
    });

    if let Err(error) = runtime.place(
        movement.placement_kind,
        &movement.new_canonical_path,
        &movement.old_canonical_path,
    ) {
        return fail_with_compensation("place-old-source", error, journal, runtime);
    }
    journal.push(UndoStep::RemoveOldSourcePlacement {
        kind: movement.placement_kind,
        source: movement.new_canonical_path.clone(),
        target: movement.old_canonical_path.clone(),
    });

    let persist_result = persist_relocation(adapter, &movement);
    if let Err(error) = persist_result {
        return fail_with_compensation("persist", error, journal, runtime);
    }

    Ok(RelocationOutcome::Moved)
}

fn preflight_relocation<M>(
    db: &Database,
    movement: &PreparedSourceMove<M>,
) -> AppResult<DestinationState> {
    let storage = movement.storage;
    let conn = db.connection()?;
    let source_role = conn
        .query_row(
            &format!(
                "SELECT role FROM {} WHERE {} = ?1 AND agent = ?2",
                storage.table(),
                storage.id_column()
            ),
            params![movement.asset_id, movement.old_source_agent],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    if source_role.as_deref() != Some("source") {
        return Err(AppError::Validation(
            "distribution source changed before relocation".to_string(),
        ));
    }
    let source_count: i64 = conn.query_row(
        &format!(
            "SELECT COUNT(*) FROM {} WHERE {} = ?1 AND role = 'source'",
            storage.table(),
            storage.id_column()
        ),
        params![movement.asset_id],
        |row| row.get(0),
    )?;
    if source_count != 1 {
        return Err(AppError::Validation(
            "distribution must have exactly one source before relocation".to_string(),
        ));
    }
    let target_distribution = conn
        .query_row(
            &format!(
                "SELECT role, target_path FROM {} WHERE {} = ?1 AND agent = ?2",
                storage.table(),
                storage.id_column()
            ),
            params![movement.asset_id, movement.new_source_agent],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .optional()?;
    drop(conn);
    if matches!(target_distribution, Some((ref role, _)) if role == "source") {
        return Err(AppError::Validation(
            "distribution target is already recorded as a source".to_string(),
        ));
    }
    if let Some((role, recorded_path)) = &target_distribution {
        if role == "target" {
            let expected_path =
                path_to_string(&movement.new_canonical_path, "distribution target path")?;
            if recorded_path.as_deref() != Some(expected_path.as_str()) {
                return Err(AppError::Validation(format!(
                    "distribution target record does not match the planned path: {}",
                    movement.new_canonical_path.display()
                )));
            }
        }
    }

    match fs::symlink_metadata(&movement.old_canonical_path) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(AppError::Validation(format!(
                "canonical source does not exist: {}",
                movement.old_canonical_path.display()
            )))
        }
        Err(error) => return Err(error.into()),
    }

    match fs::symlink_metadata(&movement.new_canonical_path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(DestinationState::Vacant),
        Err(error) => Err(error.into()),
        Ok(_) => {
            let managed = matches!(target_distribution, Some((ref role, _)) if role == "target")
                && match movement.placement_kind {
                    PlacementKind::Directory => managed_directory_placement_points_to(
                        &movement.new_canonical_path,
                        &movement.old_canonical_path,
                    )?,
                    PlacementKind::File => managed_file_placement_points_to(
                        &movement.new_canonical_path,
                        &movement.old_canonical_path,
                    )?,
                };
            if managed {
                Ok(DestinationState::ManagedPlacement)
            } else {
                Err(AppError::Validation(format!(
                    "distribution target path is occupied by unmanaged content: {}",
                    movement.new_canonical_path.display()
                )))
            }
        }
    }
}

fn persist_relocation<A: SourceRelocationAdapter>(
    adapter: &A,
    movement: &PreparedSourceMove<A::Metadata>,
) -> AppResult<()> {
    let now = now_epoch_seconds()?;
    let mut conn = adapter.database().connection()?;
    let tx = conn.transaction()?;
    validate_transaction_source(&tx, movement)?;
    adapter.persist_asset_move(&tx, movement, now)?;
    upsert_relocation_roles(&tx, movement)?;
    validate_relocated_source(&tx, movement)?;
    tx.commit()?;
    Ok(())
}

fn validate_transaction_source<M>(
    tx: &Transaction<'_>,
    movement: &PreparedSourceMove<M>,
) -> AppResult<()> {
    let table = movement.storage.table();
    let id_column = movement.storage.id_column();
    let (source_count, expected_source_count): (i64, i64) = tx.query_row(
        &format!(
            "SELECT COUNT(*) FILTER (WHERE role = 'source'),\n                    COUNT(*) FILTER (WHERE role = 'source' AND agent = ?2)\n             FROM {table} WHERE {id_column} = ?1"
        ),
        params![movement.asset_id, movement.old_source_agent],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    if source_count != 1 || expected_source_count != 1 {
        return Err(AppError::Validation(
            "distribution source changed during relocation".to_string(),
        ));
    }
    Ok(())
}

fn upsert_relocation_roles<M>(
    tx: &Transaction<'_>,
    movement: &PreparedSourceMove<M>,
) -> AppResult<()> {
    let table = movement.storage.table();
    let id_column = movement.storage.id_column();
    let old_target_path = path_to_string(&movement.old_canonical_path, "distribution target path")?;
    tx.execute(
        &format!(
            "INSERT INTO {table} ({id_column}, agent, role, target_path)\n             VALUES (?1, ?2, 'target', ?3)\n             ON CONFLICT({id_column}, agent) DO UPDATE SET\n                 role = 'target', target_path = excluded.target_path"
        ),
        params![movement.asset_id, movement.old_source_agent, old_target_path],
    )?;
    tx.execute(
        &format!(
            "INSERT INTO {table} ({id_column}, agent, role, target_path)\n             VALUES (?1, ?2, 'source', NULL)\n             ON CONFLICT({id_column}, agent) DO UPDATE SET\n                 role = 'source', target_path = NULL"
        ),
        params![movement.asset_id, movement.new_source_agent],
    )?;
    Ok(())
}

fn validate_relocated_source<M>(
    tx: &Transaction<'_>,
    movement: &PreparedSourceMove<M>,
) -> AppResult<()> {
    let table = movement.storage.table();
    let id_column = movement.storage.id_column();
    let (source_count, expected_source_count): (i64, i64) = tx.query_row(
        &format!(
            "SELECT COUNT(*) FILTER (WHERE role = 'source'),\n                    COUNT(*) FILTER (WHERE role = 'source' AND agent = ?2)\n             FROM {table} WHERE {id_column} = ?1"
        ),
        params![movement.asset_id, movement.new_source_agent],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    if source_count != 1 || expected_source_count != 1 {
        return Err(AppError::Validation(
            "distribution relocation did not produce exactly one source".to_string(),
        ));
    }
    Ok(())
}

fn fail_with_compensation(
    stage: &str,
    original: AppError,
    journal: Vec<UndoStep>,
    runtime: &impl RelocationRuntime,
) -> AppResult<RelocationOutcome> {
    let mut failures = Vec::new();
    for step in journal.iter().rev() {
        if let Err(error) = step.execute(runtime) {
            failures.push(format!("{} [{}]: {}", step.label(), step.paths(), error));
        }
    }
    if failures.is_empty() {
        Err(original)
    } else {
        Err(AppError::Reconciliation(format!(
            "source relocation failed at {stage}: {original}; compensation failures: {}; old/new paths are included above",
            failures.join("; ")
        )))
    }
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

        SELECT skill.canonical_path, distribution.target_path
        FROM skills skill
        JOIN skill_project_distributions distribution ON distribution.skill_id = skill.id
        WHERE skill.scope = 'project'
          AND skill.source_kind = 'project_custom'
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
pub fn placement_points_to(target_path: &Path, source_path: &Path) -> AppResult<bool> {
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
    let resolved_target = match target_path.canonicalize() {
        Ok(path) => path,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    let resolved_source = match source_path.canonicalize() {
        Ok(path) => path,
        // A stale distribution must not make inventory scanning fail or hide a
        // replacement link just because its recorded source was removed.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    Ok(resolved_target == resolved_source)
}

#[cfg(test)]
mod relocation_tests {
    use std::{cell::RefCell, collections::HashSet};

    use super::*;
    use crate::services::agent_capabilities::agent_by_name;

    #[derive(Clone, Copy)]
    enum PersistMode {
        Normal,
        AssetFailure,
        RoleFailure,
        ExtraSource,
    }

    struct TestAdapter {
        db: Database,
        old_path: PathBuf,
        new_path: PathBuf,
        unchanged: bool,
        persist_mode: PersistMode,
    }

    impl SourceRelocationAdapter for TestAdapter {
        type Metadata = ();

        fn database(&self) -> &Database {
            &self.db
        }

        fn plan_relocation(
            &self,
            asset_id: &str,
            _target_agent: &'static AgentCapabilitySurface,
        ) -> AppResult<RelocationPlan<Self::Metadata>> {
            if self.unchanged {
                return Ok(RelocationPlan::Unchanged);
            }
            Ok(RelocationPlan::Move(PreparedSourceMove {
                asset_id: asset_id.to_string(),
                storage: DistributionStorage::Skill,
                old_source_agent: "Generic Agent",
                new_source_agent: "CodeX",
                old_canonical_path: self.old_path.clone(),
                new_canonical_path: self.new_path.clone(),
                placement_kind: PlacementKind::Directory,
                metadata: (),
            }))
        }

        fn persist_asset_move(
            &self,
            tx: &Transaction<'_>,
            movement: &PreparedSourceMove<Self::Metadata>,
            now: i64,
        ) -> AppResult<()> {
            match self.persist_mode {
                PersistMode::AssetFailure => {
                    return Err(AppError::Validation("injected asset failure".to_string()))
                }
                PersistMode::RoleFailure => {
                    tx.execute("DROP TABLE skill_distributions", [])?;
                }
                PersistMode::ExtraSource => {
                    tx.execute(
                        "INSERT INTO skill_distributions (skill_id, agent, role, target_path) VALUES (?1, 'Claude Code', 'source', NULL)",
                        params![movement.asset_id],
                    )?;
                }
                PersistMode::Normal => {}
            }
            tx.execute(
                "UPDATE skills SET canonical_path = ?2, source_agent = ?3, updated_at = ?4 WHERE id = ?1",
                params![
                    movement.asset_id,
                    path_to_string(&movement.new_canonical_path, "test skill path")?,
                    movement.new_source_agent,
                    now
                ],
            )?;
            Ok(())
        }
    }

    fn fixture(
        managed_target: bool,
        persist_mode: PersistMode,
    ) -> (tempfile::TempDir, TestAdapter) {
        let root = tempfile::TempDir::new().expect("temp dir");
        let old_path = root.path().join("generic/test-skill");
        let new_path = root.path().join("codex/test-skill");
        fs::create_dir_all(&old_path).expect("old source");
        fs::write(old_path.join("SKILL.md"), "# Test\n").expect("skill body");
        let db = Database::open_in_memory().expect("database");
        {
            let conn = db.connection().expect("connection");
            conn.execute(
                "INSERT INTO skills (id, name, scope, project_id, description, canonical_path, disabled, source_kind, source_agent, created_at, updated_at) VALUES ('asset', 'test-skill', 'global', NULL, '', ?1, 0, 'agent', 'Generic Agent', 1, 1)",
                params![path_to_string(&old_path, "old path").expect("path")],
            )
            .expect("skill");
            conn.execute(
                "INSERT INTO skill_distributions (skill_id, agent, role, target_path) VALUES ('asset', 'Generic Agent', 'source', NULL)",
                [],
            )
            .expect("source role");
        }
        if managed_target {
            create_managed_directory_link(&old_path, &new_path).expect("managed target");
            let conn = db.connection().expect("connection");
            conn.execute(
                "INSERT INTO skill_distributions (skill_id, agent, role, target_path) VALUES ('asset', 'CodeX', 'target', ?1)",
                params![path_to_string(&new_path, "target path").expect("path")],
            )
            .expect("target role");
        }
        (
            root,
            TestAdapter {
                db,
                old_path,
                new_path,
                unchanged: false,
                persist_mode,
            },
        )
    }

    fn target_agent() -> &'static AgentCapabilitySurface {
        agent_by_name("CodeX").expect("CodeX")
    }

    fn assert_original_state(adapter: &TestAdapter, managed_target: bool) {
        assert!(adapter.old_path.join("SKILL.md").is_file());
        if managed_target {
            assert!(placement_points_to(&adapter.new_path, &adapter.old_path).expect("placement"));
        } else {
            assert!(!adapter.new_path.exists());
        }
        let conn = adapter.db.connection().expect("connection");
        let canonical: String = conn
            .query_row(
                "SELECT canonical_path FROM skills WHERE id = 'asset'",
                [],
                |row| row.get(0),
            )
            .expect("canonical");
        assert_eq!(Path::new(&canonical), adapter.old_path);
        let source: String = conn
            .query_row(
                "SELECT agent FROM skill_distributions WHERE skill_id = 'asset' AND role = 'source'",
                [],
                |row| row.get(0),
            )
            .expect("source");
        assert_eq!(source, "Generic Agent");
    }

    #[test]
    fn relocation_noop_does_not_mutate() {
        let (_root, mut adapter) = fixture(false, PersistMode::Normal);
        adapter.unchanged = true;
        let outcome = relocate_source(&adapter, "asset", target_agent()).expect("no-op");
        assert_eq!(outcome, RelocationOutcome::Unchanged);
        assert_original_state(&adapter, false);
    }

    #[test]
    fn relocation_moves_vacant_or_managed_target_and_switches_roles() {
        for managed_target in [false, true] {
            let (_root, adapter) = fixture(managed_target, PersistMode::Normal);
            let outcome = relocate_source(&adapter, "asset", target_agent()).expect("relocate");
            assert_eq!(outcome, RelocationOutcome::Moved);
            assert!(adapter.new_path.join("SKILL.md").is_file());
            assert!(placement_points_to(&adapter.old_path, &adapter.new_path).expect("old target"));
            let conn = adapter.db.connection().expect("connection");
            let roles = cells(&conn, "skill_distributions", "skill_id", "asset").expect("roles");
            assert_eq!(roles["Generic Agent"], "target");
            assert_eq!(roles["CodeX"], "source");
            assert_eq!(
                roles
                    .values()
                    .filter(|role| role.as_str() == "source")
                    .count(),
                1
            );
        }
    }

    #[test]
    fn relocation_rejects_unmanaged_conflict_before_mutation() {
        let (_root, adapter) = fixture(false, PersistMode::Normal);
        fs::create_dir_all(&adapter.new_path).expect("conflict");
        fs::write(adapter.new_path.join("owned.txt"), "mine").expect("conflict body");
        let error = relocate_source(&adapter, "asset", target_agent()).expect_err("conflict");
        assert!(matches!(error, AppError::Validation(_)));
        assert!(adapter.new_path.join("owned.txt").is_file());
        fs::remove_dir_all(&adapter.new_path).expect("remove test conflict");
        assert_original_state(&adapter, false);
    }

    #[test]
    fn relocation_rejects_a_target_record_for_another_path() {
        let (_root, adapter) = fixture(true, PersistMode::Normal);
        let conn = adapter.db.connection().expect("connection");
        conn.execute(
            "UPDATE skill_distributions SET target_path = ?1 WHERE skill_id = 'asset' AND agent = 'CodeX'",
            params![path_to_string(&adapter.new_path.with_extension("stale"), "stale path")
                .expect("path")],
        )
        .expect("stale target record");
        drop(conn);

        let error = relocate_source(&adapter, "asset", target_agent()).expect_err("stale target");

        assert!(matches!(error, AppError::Validation(_)));
        assert_original_state(&adapter, true);
    }

    #[test]
    fn relocation_compensates_asset_and_role_database_failures() {
        for mode in [
            PersistMode::AssetFailure,
            PersistMode::RoleFailure,
            PersistMode::ExtraSource,
        ] {
            let (_root, adapter) = fixture(true, mode);
            let error = relocate_source(&adapter, "asset", target_agent()).expect_err("failure");
            assert!(!matches!(error, AppError::Reconciliation(_)));
            assert_original_state(&adapter, true);
        }
    }

    struct ScriptedRuntime {
        log: RefCell<Vec<&'static str>>,
        failures: HashSet<&'static str>,
    }

    impl ScriptedRuntime {
        fn record(&self, step: &'static str) -> AppResult<()> {
            self.log.borrow_mut().push(step);
            if self.failures.contains(step) {
                Err(AppError::Io(format!("injected {step}")))
            } else {
                Ok(())
            }
        }
    }

    impl RelocationRuntime for ScriptedRuntime {
        fn create_parents(&self, _target: &Path) -> AppResult<Vec<PathBuf>> {
            self.record("create-parents")?;
            Ok(Vec::new())
        }

        fn rename(&self, source: &Path, _target: &Path) -> AppResult<()> {
            let step = if source.file_name().and_then(|name| name.to_str()) == Some("test-skill")
                && source
                    .parent()
                    .and_then(Path::file_name)
                    .and_then(|name| name.to_str())
                    == Some("generic")
            {
                "rename-forward"
            } else {
                "rename-back"
            };
            self.record(step)
        }

        fn place(&self, _kind: PlacementKind, _source: &Path, target: &Path) -> AppResult<()> {
            let step = if target
                .parent()
                .and_then(Path::file_name)
                .and_then(|name| name.to_str())
                == Some("generic")
            {
                "place-old-source"
            } else {
                "restore-target"
            };
            self.record(step)
        }

        fn remove(&self, _kind: PlacementKind, _source: &Path, target: &Path) -> AppResult<()> {
            let step = if target
                .parent()
                .and_then(Path::file_name)
                .and_then(|name| name.to_str())
                == Some("codex")
            {
                "remove-target"
            } else {
                "remove-old-source"
            };
            self.record(step)
        }
    }

    struct FaultingFilesystemRuntime {
        filesystem: FilesystemRelocationRuntime,
        failure: &'static str,
        log: RefCell<Vec<&'static str>>,
    }

    impl FaultingFilesystemRuntime {
        fn run(
            &self,
            step: &'static str,
            operation: impl FnOnce() -> AppResult<()>,
        ) -> AppResult<()> {
            self.log.borrow_mut().push(step);
            if self.failure == step {
                Err(AppError::Io(format!("injected {step}")))
            } else {
                operation()
            }
        }
    }

    impl RelocationRuntime for FaultingFilesystemRuntime {
        fn create_parents(&self, target: &Path) -> AppResult<Vec<PathBuf>> {
            self.log.borrow_mut().push("create-parents");
            self.filesystem.create_parents(target)
        }

        fn rename(&self, source: &Path, target: &Path) -> AppResult<()> {
            let step = if source
                .parent()
                .and_then(Path::file_name)
                .and_then(|name| name.to_str())
                == Some("generic")
            {
                "rename-forward"
            } else {
                "rename-back"
            };
            self.run(step, || self.filesystem.rename(source, target))
        }

        fn place(&self, kind: PlacementKind, source: &Path, target: &Path) -> AppResult<()> {
            let step = if target
                .parent()
                .and_then(Path::file_name)
                .and_then(|name| name.to_str())
                == Some("generic")
            {
                "place-old-source"
            } else {
                "restore-target"
            };
            self.run(step, || self.filesystem.place(kind, source, target))
        }

        fn remove(&self, kind: PlacementKind, source: &Path, target: &Path) -> AppResult<()> {
            let step = if target
                .parent()
                .and_then(Path::file_name)
                .and_then(|name| name.to_str())
                == Some("codex")
            {
                "remove-target"
            } else {
                "remove-old-source"
            };
            self.run(step, || self.filesystem.remove(kind, source, target))
        }
    }

    #[test]
    fn relocation_compensates_rename_and_old_source_placement_failures() {
        let cases = [
            ("remove-target", vec!["remove-target"]),
            (
                "rename-forward",
                vec![
                    "remove-target",
                    "create-parents",
                    "rename-forward",
                    "restore-target",
                ],
            ),
            (
                "place-old-source",
                vec![
                    "remove-target",
                    "create-parents",
                    "rename-forward",
                    "place-old-source",
                    "rename-back",
                    "restore-target",
                ],
            ),
        ];
        for (failure, expected) in cases {
            let (_root, adapter) = fixture(true, PersistMode::Normal);
            let runtime = FaultingFilesystemRuntime {
                filesystem: FilesystemRelocationRuntime,
                failure,
                log: RefCell::new(Vec::new()),
            };
            let error = relocate_source_with_runtime(&adapter, "asset", target_agent(), &runtime)
                .expect_err("injected filesystem failure");
            assert!(matches!(error, AppError::Io(_)));
            assert_eq!(*runtime.log.borrow(), expected);
            assert_original_state(&adapter, true);
        }
    }

    #[test]
    fn relocation_journal_is_reverse_order_and_reports_compensation_failure() {
        let (_root, adapter) = fixture(true, PersistMode::AssetFailure);
        let runtime = ScriptedRuntime {
            log: RefCell::new(Vec::new()),
            failures: HashSet::from(["remove-old-source"]),
        };
        let error = relocate_source_with_runtime(&adapter, "asset", target_agent(), &runtime)
            .expect_err("reconciliation");
        let AppError::Reconciliation(message) = error else {
            panic!("expected reconciliation error");
        };
        assert!(message.contains("persist"));
        assert!(message.contains("remove-old-source-placement"));
        assert_eq!(
            runtime.log.borrow().as_slice(),
            [
                "remove-target",
                "create-parents",
                "rename-forward",
                "place-old-source",
                "remove-old-source",
                "rename-back",
                "restore-target",
            ]
        );
    }
}
