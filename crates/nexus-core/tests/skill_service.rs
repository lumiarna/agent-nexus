use std::{
    collections::BTreeMap,
    env, fs,
    path::Path,
    sync::{Arc, Barrier},
    thread,
};

use nexus_core::{
    database::Database,
    error::AppError,
    services::{
        app_config::{AgentDisplayPreferences, AppConfigService},
        paths,
        projects::ProjectService,
        skills::{
            AgentCellRole, MoveSkillSourceInput, PlacementCellRole, ProjectCustomDestinationState,
            ProjectCustomSkillDestination, ProjectCustomSkillIntent, SetSkillTargetInput,
            SkillContext, SkillRow, SkillService,
        },
        symlink::create_managed_directory_link,
    },
};
use serial_test::serial;
use tempfile::TempDir;

fn git_repo(parent: &TempDir, name: &str) -> String {
    let path = parent.path().join(name);
    fs::create_dir_all(path.join(".git")).expect("create test git repo");
    path.to_string_lossy().into_owned()
}

fn write_skill(dir: &Path, body: &str) {
    fs::create_dir_all(dir).expect("create skill dir");
    fs::write(dir.join("SKILL.md"), body).expect("write SKILL.md");
}

fn service(db: Arc<Database>) -> SkillService {
    SkillService::new(db.clone(), AppConfigService::new(db))
}

fn isolate_home(root: &TempDir) -> std::path::PathBuf {
    let home = root.path().join("home");
    fs::create_dir_all(&home).expect("create isolated home");
    env::set_var("HOME", &home);
    env::set_var("USERPROFILE", &home);
    home
}

fn skill_id(row: &SkillRow) -> &str {
    &row.skill().skill_id
}

fn custom_canonical(rows: &[SkillRow]) -> &SkillRow {
    rows.iter()
        .find(|row| matches!(row, SkillRow::ProjectCustomCanonical { .. }))
        .expect("Project custom canonical row")
}

fn incoming_for<'a>(rows: &'a [SkillRow], project_id: &str) -> Option<&'a SkillRow> {
    rows.iter().find(|row| {
        matches!(
            row,
            SkillRow::ProjectCustomIncoming { target_project, .. }
                if target_project.id == project_id
        )
    })
}

fn destination<'a>(
    row: &'a SkillRow,
    expected: &ProjectCustomSkillDestination,
) -> &'a BTreeMap<String, PlacementCellRole> {
    let SkillRow::ProjectCustomCanonical { destinations, .. } = row else {
        panic!("expected Project custom canonical row")
    };
    destinations
        .iter()
        .find_map(|state| match (state, expected) {
            (
                ProjectCustomDestinationState::Global { cells },
                ProjectCustomSkillDestination::Global,
            ) => Some(cells),
            (
                ProjectCustomDestinationState::Project { project, cells },
                ProjectCustomSkillDestination::Project { project_id },
            ) if project.id == *project_id => Some(cells),
            _ => None,
        })
        .expect("destination state")
}

fn assert_link_points_to(source: &Path, target: &Path) {
    assert_eq!(
        fs::canonicalize(target).expect("canonicalize target"),
        fs::canonicalize(source).expect("canonicalize source")
    );
}

fn apply(
    service: &SkillService,
    skill_id: &str,
    destination: ProjectCustomSkillDestination,
    enabled: bool,
) -> Vec<SkillRow> {
    service
        .apply_project_custom_skill_intent(ProjectCustomSkillIntent::SetTargetEnabled {
            skill_id: skill_id.to_string(),
            destination,
            enabled,
        })
        .expect("apply target intent")
        .skills
}

fn set_agent(
    service: &SkillService,
    skill_id: &str,
    destination: ProjectCustomSkillDestination,
    agent: &str,
    enabled: bool,
) -> Vec<SkillRow> {
    service
        .apply_project_custom_skill_intent(ProjectCustomSkillIntent::SetAgentPlacement {
            skill_id: skill_id.to_string(),
            destination,
            agent: agent.to_string(),
            enabled,
        })
        .expect("apply placement intent")
        .skills
}

#[test]
fn skill_row_json_contract_is_camel_case_and_separates_cell_roles() {
    let mut agent_cells = BTreeMap::new();
    agent_cells.insert("Generic Agent".to_string(), AgentCellRole::Source);
    let row = SkillRow::AgentCanonical {
        row_key: "row-1".to_string(),
        skill: nexus_core::services::skills::SkillSummary {
            skill_id: "skill-1".to_string(),
            name: "demo".to_string(),
            desc: "Demo".to_string(),
            path: "/demo".to_string(),
            disabled: false,
        },
        context: SkillContext::Global,
        source_agent: "Generic Agent".to_string(),
        cells: agent_cells,
    };
    let json = serde_json::to_value(row).expect("serialize Skill row");
    assert_eq!(json["kind"], "agentCanonical");
    assert_eq!(json["rowKey"], "row-1");
    assert_eq!(json["skill"]["skillId"], "skill-1");
    assert_eq!(json["sourceAgent"], "Generic Agent");
    assert_eq!(json["cells"]["Generic Agent"], "source");

    let placement = serde_json::to_value(ProjectCustomDestinationState::Global {
        cells: BTreeMap::from([("Generic Agent".to_string(), PlacementCellRole::Target)]),
    })
    .expect("serialize destination");
    assert_eq!(placement["kind"], "global");
    assert_eq!(placement["cells"]["Generic Agent"], "target");
    assert_ne!(placement["cells"]["Generic Agent"], "source");

    let intent = serde_json::to_value(ProjectCustomSkillIntent::SetAgentPlacement {
        skill_id: "skill-1".to_string(),
        destination: ProjectCustomSkillDestination::Project {
            project_id: "project-1".to_string(),
        },
        agent: "CodeX".to_string(),
        enabled: true,
    })
    .expect("serialize intent");
    assert_eq!(intent["kind"], "setAgentPlacement");
    assert_eq!(intent["skillId"], "skill-1");
    assert_eq!(intent["destination"]["kind"], "project");
    assert_eq!(intent["destination"]["projectId"], "project-1");

    let reconciliation = serde_json::to_value(AppError::Reconciliation("evidence e1".to_string()))
        .expect("serialize reconciliation error");
    assert_eq!(reconciliation["kind"], "reconciliation");
    assert_eq!(reconciliation["message"], "evidence e1");
}

#[test]
#[serial]
fn scan_returns_explicit_rows_and_eager_effective_destinations() {
    let db = Arc::new(Database::open_in_memory().expect("open database"));
    let projects = ProjectService::new(db.clone());
    let skills = service(db.clone());
    let root = TempDir::new().expect("temp dir");
    isolate_home(&root);
    let source_repo = git_repo(&root, "source");
    let target_repo = git_repo(&root, "target");
    let missing_repo = git_repo(&root, "missing");
    let source = projects
        .record_project(source_repo.clone())
        .expect("source");
    let target = projects.record_project(target_repo).expect("target");
    let missing = projects
        .record_project(missing_repo.clone())
        .expect("missing");
    fs::remove_dir_all(missing_repo).expect("remove missing Project Path");

    write_skill(
        &Path::new(&source_repo).join("skills/release-notes"),
        "---\nname: release-notes\ndescription: notes\n---\n",
    );
    let rows = skills.scan_skills().expect("scan");
    let canonical = custom_canonical(&rows);
    let SkillRow::ProjectCustomCanonical {
        row_key,
        skill,
        source_project,
        destinations,
    } = canonical
    else {
        unreachable!()
    };
    assert_eq!(row_key, &skill.skill_id);
    assert_eq!(source_project.id, source.id);
    assert_eq!(destinations.len(), 3, "Global + source + healthy target");
    assert!(matches!(
        destinations[0],
        ProjectCustomDestinationState::Global { .. }
    ));
    assert!(destination(
        canonical,
        &ProjectCustomSkillDestination::Project {
            project_id: source.id
        }
    )
    .values()
    .all(|role| *role == PlacementCellRole::None));
    assert!(destination(
        canonical,
        &ProjectCustomSkillDestination::Project {
            project_id: target.id
        }
    )
    .values()
    .all(|role| *role == PlacementCellRole::None));
    assert!(!destinations.iter().any(|state| matches!(
        state,
        ProjectCustomDestinationState::Project { project, .. } if project.id == missing.id
    )));
}

#[test]
#[serial]
fn global_intent_uses_backend_default_fans_out_withdraws_all_and_is_idempotent() {
    let db = Arc::new(Database::open_in_memory().expect("open database"));
    let projects = ProjectService::new(db.clone());
    let config = AppConfigService::new(db.clone());
    config
        .set_agent_display_preferences(&AgentDisplayPreferences {
            disabled: vec![],
            default_global_entry_agent: Some("Claude Code".to_string()),
        })
        .expect("set default Agent");
    let skills = SkillService::new(db, config);
    let root = TempDir::new().expect("temp dir");
    let home = isolate_home(&root);
    let repo = git_repo(&root, "source");
    projects
        .record_project(repo.clone())
        .expect("record Project");
    let source_path = Path::new(&repo).join("skills/release-notes");
    write_skill(&source_path, "---\nname: release-notes\n---\n");
    let scanned = skills.scan_skills().expect("scan");
    let id = skill_id(custom_canonical(&scanned)).to_string();

    let enabled = skills
        .apply_project_custom_skill_intent(ProjectCustomSkillIntent::SetTargetEnabled {
            skill_id: id.clone(),
            destination: ProjectCustomSkillDestination::Global,
            enabled: true,
        })
        .expect("enable Global");
    assert!(enabled.changed);
    assert_eq!(
        destination(
            custom_canonical(&enabled.skills),
            &ProjectCustomSkillDestination::Global
        )["Claude Code"],
        PlacementCellRole::Target
    );
    assert_link_points_to(&source_path, &home.join(".claude/skills/release-notes"));

    let repeated = skills
        .apply_project_custom_skill_intent(ProjectCustomSkillIntent::SetTargetEnabled {
            skill_id: id.clone(),
            destination: ProjectCustomSkillDestination::Global,
            enabled: true,
        })
        .expect("repeat enable");
    assert!(!repeated.changed);

    set_agent(
        &skills,
        &id,
        ProjectCustomSkillDestination::Global,
        "CodeX",
        true,
    );
    let rescanned = skills
        .scan_skills()
        .expect("rescan managed Global placements");
    assert_eq!(
        rescanned
            .iter()
            .filter(|row| !matches!(row, SkillRow::ProjectCustomIncoming { .. }))
            .count(),
        1,
        "managed Global placements do not become canonical Skills",
    );
    let removed = apply(&skills, &id, ProjectCustomSkillDestination::Global, false);
    assert!(destination(
        custom_canonical(&removed),
        &ProjectCustomSkillDestination::Global
    )
    .values()
    .all(|role| *role == PlacementCellRole::None));
    assert!(!home.join(".claude/skills/release-notes").exists());
    assert!(!home.join(".codex/skills/release-notes").exists());
}

#[test]
#[serial]
fn project_intent_supports_source_and_cross_project_fanout_and_last_removal() {
    let db = Arc::new(Database::open_in_memory().expect("open database"));
    let projects = ProjectService::new(db.clone());
    let skills = service(db);
    let root = TempDir::new().expect("temp dir");
    isolate_home(&root);
    let source_repo = git_repo(&root, "source");
    let target_repo = git_repo(&root, "target");
    let source = projects
        .record_project(source_repo.clone())
        .expect("source");
    let target = projects
        .record_project(target_repo.clone())
        .expect("target");
    let source_path = Path::new(&source_repo).join("skills/release-notes");
    write_skill(&source_path, "---\nname: release-notes\n---\n");
    let rows = skills.scan_skills().expect("scan");
    let id = skill_id(custom_canonical(&rows)).to_string();

    let source_destination = ProjectCustomSkillDestination::Project {
        project_id: source.id.clone(),
    };
    let target_destination = ProjectCustomSkillDestination::Project {
        project_id: target.id.clone(),
    };
    apply(&skills, &id, source_destination.clone(), true);
    let target_rows = apply(&skills, &id, target_destination.clone(), true);
    assert!(incoming_for(&target_rows, &source.id).is_some());
    assert!(incoming_for(&target_rows, &target.id).is_some());

    let fanned = set_agent(&skills, &id, target_destination.clone(), "CodeX", true);
    let SkillRow::ProjectCustomIncoming { cells, .. } =
        incoming_for(&fanned, &target.id).expect("incoming row")
    else {
        unreachable!()
    };
    assert_eq!(cells["Generic Agent"], PlacementCellRole::Target);
    assert_eq!(cells["CodeX"], PlacementCellRole::Target);
    assert_link_points_to(
        &source_path,
        &Path::new(&target_repo).join(".codex/skills/release-notes"),
    );
    let project_rows = projects.list_projects().expect("Project counts");
    assert_eq!(
        project_rows
            .iter()
            .find(|project| project.id == target.id)
            .expect("target Project")
            .skills,
        1,
        "incoming row contributes to target Project Skill count",
    );

    let one_left = set_agent(
        &skills,
        &id,
        target_destination.clone(),
        "Generic Agent",
        false,
    );
    assert!(incoming_for(&one_left, &target.id).is_some());
    let gone = set_agent(&skills, &id, target_destination, "CodeX", false);
    assert!(incoming_for(&gone, &target.id).is_none());
    let project_rows = projects
        .list_projects()
        .expect("Project counts after removal");
    assert_eq!(
        project_rows
            .iter()
            .find(|project| project.id == target.id)
            .expect("target Project")
            .skills,
        0,
    );
}

#[test]
#[serial]
fn stale_hidden_and_missing_path_projects_are_rejected_without_creating_targets() {
    let db = Arc::new(Database::open_in_memory().expect("open database"));
    let projects = ProjectService::new(db.clone());
    let skills = service(db.clone());
    let root = TempDir::new().expect("temp dir");
    isolate_home(&root);
    let source_repo = git_repo(&root, "source");
    let stale_repo = git_repo(&root, "stale");
    let hidden_repo = git_repo(&root, "hidden");
    projects
        .record_project(source_repo.clone())
        .expect("source");
    let stale = projects.record_project(stale_repo.clone()).expect("stale");
    let hidden = projects
        .record_project(hidden_repo.clone())
        .expect("hidden");
    {
        let conn = db.connection().expect("connection");
        conn.execute(
            "UPDATE projects SET status = 'hidden' WHERE id = ?1",
            [&hidden.id],
        )
        .expect("hide Project");
    }
    fs::remove_dir_all(&stale_repo).expect("remove stale path");
    let source_path = Path::new(&source_repo).join("skills/release-notes");
    write_skill(&source_path, "---\nname: release-notes\n---\n");
    let rows = skills.scan_skills().expect("scan");
    let id = skill_id(custom_canonical(&rows)).to_string();

    for project_id in [stale.id, hidden.id] {
        let error = skills
            .apply_project_custom_skill_intent(ProjectCustomSkillIntent::SetTargetEnabled {
                skill_id: id.clone(),
                destination: ProjectCustomSkillDestination::Project { project_id },
                enabled: true,
            })
            .expect_err("ineffective Project rejected");
        assert!(matches!(error, AppError::Validation(_)));
    }
    assert!(
        !Path::new(&stale_repo).exists(),
        "intent did not recreate stale root"
    );
}

#[test]
#[serial]
fn target_conflict_is_preflighted_without_database_or_filesystem_partial_success() {
    let db = Arc::new(Database::open_in_memory().expect("open database"));
    let projects = ProjectService::new(db.clone());
    let skills = service(db);
    let root = TempDir::new().expect("temp dir");
    isolate_home(&root);
    let source_repo = git_repo(&root, "source");
    let target_repo = git_repo(&root, "target");
    projects
        .record_project(source_repo.clone())
        .expect("source");
    let target = projects
        .record_project(target_repo.clone())
        .expect("target");
    let source_path = Path::new(&source_repo).join("skills/release-notes");
    write_skill(&source_path, "---\nname: release-notes\n---\n");
    let conflict = Path::new(&target_repo).join(".agents/skills/release-notes");
    write_skill(&conflict, "---\nname: conflict\n---\n");
    let rows = skills.scan_skills().expect("scan");
    let id = skill_id(custom_canonical(&rows)).to_string();

    let error = skills
        .apply_project_custom_skill_intent(ProjectCustomSkillIntent::SetTargetEnabled {
            skill_id: id.clone(),
            destination: ProjectCustomSkillDestination::Project {
                project_id: target.id.clone(),
            },
            enabled: true,
        })
        .expect_err("conflict rejected");
    assert!(error.to_string().contains("conflicting content"));
    assert!(conflict.join("SKILL.md").is_file());
    assert!(incoming_for(&skills.list_skills().expect("catalog"), &target.id).is_none());
}

#[test]
#[serial]
fn managed_project_placement_never_becomes_a_canonical_skill_on_rescan() {
    let db = Arc::new(Database::open_in_memory().expect("open database"));
    let projects = ProjectService::new(db.clone());
    let skills = service(db);
    let root = TempDir::new().expect("temp dir");
    isolate_home(&root);
    let source_repo = git_repo(&root, "source");
    let target_repo = git_repo(&root, "target");
    projects
        .record_project(source_repo.clone())
        .expect("source");
    let target = projects.record_project(target_repo).expect("target");
    write_skill(
        &Path::new(&source_repo).join("skills/release-notes"),
        "---\nname: release-notes\n---\n",
    );
    let rows = skills.scan_skills().expect("scan");
    let id = skill_id(custom_canonical(&rows)).to_string();
    apply(
        &skills,
        &id,
        ProjectCustomSkillDestination::Project {
            project_id: target.id,
        },
        true,
    );
    let rescanned = skills.scan_skills().expect("rescan");
    assert_eq!(
        rescanned
            .iter()
            .filter(|row| !matches!(row, SkillRow::ProjectCustomIncoming { .. }))
            .count(),
        1
    );
}

#[test]
#[serial]
fn retained_mutations_return_authoritative_catalog_and_reject_custom_target_command() {
    let db = Arc::new(Database::open_in_memory().expect("open database"));
    let projects = ProjectService::new(db.clone());
    let skills = service(db);
    let root = TempDir::new().expect("temp dir");
    isolate_home(&root);
    let repo = git_repo(&root, "source");
    projects.record_project(repo.clone()).expect("Project");
    let agent_source = Path::new(&repo).join(".github/skills/agent-skill");
    let custom_source = Path::new(&repo).join("skills/custom-skill");
    write_skill(&agent_source, "---\nname: agent-skill\n---\n");
    write_skill(&custom_source, "---\nname: custom-skill\n---\n");
    let rows = skills.scan_skills().expect("scan");
    let agent_id = rows
        .iter()
        .find_map(|row| match row {
            SkillRow::AgentCanonical { skill, .. } => Some(skill.skill_id.clone()),
            _ => None,
        })
        .expect("agent row");
    let custom_id = skill_id(custom_canonical(&rows)).to_string();

    let catalog = skills
        .set_skill_target(SetSkillTargetInput {
            skill_id: agent_id.clone(),
            agent: "CodeX".to_string(),
            enabled: true,
        })
        .expect("set target");
    assert_eq!(catalog.len(), 2);
    let moved = skills
        .move_skill_source(MoveSkillSourceInput {
            skill_id: agent_id,
            agent: "CodeX".to_string(),
        })
        .expect("move source");
    assert_eq!(moved.len(), 2);

    let error = skills
        .set_skill_target(SetSkillTargetInput {
            skill_id: custom_id,
            agent: "CodeX".to_string(),
            enabled: true,
        })
        .expect_err("custom Skill requires intent");
    assert!(error.to_string().contains("propagation intent"));
}

#[test]
#[serial]
fn dmi_updates_canonical_summary_and_skill_file() {
    let db = Arc::new(Database::open_in_memory().expect("open database"));
    let projects = ProjectService::new(db.clone());
    let skills = service(db);
    let root = TempDir::new().expect("temp dir");
    isolate_home(&root);
    let repo = git_repo(&root, "source");
    projects.record_project(repo.clone()).expect("Project");
    let source = Path::new(&repo).join("skills/custom-skill");
    write_skill(&source, "---\nname: custom-skill\n---\n");
    let rows = skills.scan_skills().expect("scan");
    let id = skill_id(custom_canonical(&rows)).to_string();
    let updated = skills.set_skill_disabled(id, true).expect("set disabled");
    assert!(custom_canonical(&updated).skill().disabled);
    assert!(fs::read_to_string(source.join("SKILL.md"))
        .expect("read Skill")
        .contains("disable-model-invocation: true"));
}

#[test]
#[serial]
fn scan_skills_errors_when_home_is_unset() {
    let db = Arc::new(Database::open_in_memory().expect("open database"));
    let skills = service(db);
    let previous_home = env::var_os("HOME");
    let previous_userprofile = env::var_os("USERPROFILE");
    env::remove_var("HOME");
    env::remove_var("USERPROFILE");
    let result = skills.scan_skills();
    match previous_home {
        Some(value) => env::set_var("HOME", value),
        None => env::remove_var("HOME"),
    }
    match previous_userprofile {
        Some(value) => env::set_var("USERPROFILE", value),
        None => env::remove_var("USERPROFILE"),
    }
    assert!(result
        .expect_err("scan fails")
        .to_string()
        .contains("cannot resolve '~'"));
}

#[test]
#[serial]
fn preexisting_correct_placement_is_an_idempotent_completed_step() {
    let db = Arc::new(Database::open_in_memory().expect("open database"));
    let projects = ProjectService::new(db.clone());
    let skills = service(db);
    let root = TempDir::new().expect("temp dir");
    isolate_home(&root);
    let source_repo = git_repo(&root, "source");
    let target_repo = git_repo(&root, "target");
    projects
        .record_project(source_repo.clone())
        .expect("source");
    let target = projects
        .record_project(target_repo.clone())
        .expect("target");
    let source = Path::new(&source_repo).join("skills/release-notes");
    write_skill(&source, "---\nname: release-notes\n---\n");
    let target_path = Path::new(&target_repo).join(".agents/skills/release-notes");
    create_managed_directory_link(&source, &target_path).expect("precreate correct placement");
    let rows = skills.scan_skills().expect("scan");
    let id = skill_id(custom_canonical(&rows)).to_string();
    let result = skills
        .apply_project_custom_skill_intent(ProjectCustomSkillIntent::SetTargetEnabled {
            skill_id: id,
            destination: ProjectCustomSkillDestination::Project {
                project_id: target.id.clone(),
            },
            enabled: true,
        })
        .expect("converge database to existing placement");
    assert!(result.changed);
    assert!(incoming_for(&result.skills, &target.id).is_some());
}

#[test]
#[serial]
fn scan_and_project_custom_intent_share_one_mutation_critical_section() {
    let db = Arc::new(Database::open_in_memory().expect("open database"));
    let projects = ProjectService::new(db.clone());
    let skills = Arc::new(service(db));
    let root = TempDir::new().expect("temp dir");
    isolate_home(&root);
    let repo = git_repo(&root, "source");
    projects.record_project(repo.clone()).expect("Project");
    write_skill(
        &Path::new(&repo).join("skills/concurrent"),
        "---\nname: concurrent\n---\n",
    );
    let rows = skills.scan_skills().expect("initial scan");
    let id = skill_id(custom_canonical(&rows)).to_string();
    let barrier = Arc::new(Barrier::new(3));

    let scanner = {
        let skills = skills.clone();
        let barrier = barrier.clone();
        thread::spawn(move || {
            barrier.wait();
            skills.scan_skills().expect("concurrent scan");
        })
    };
    let writer = {
        let skills = skills.clone();
        let barrier = barrier.clone();
        thread::spawn(move || {
            barrier.wait();
            skills
                .apply_project_custom_skill_intent(ProjectCustomSkillIntent::SetAgentPlacement {
                    skill_id: id,
                    destination: ProjectCustomSkillDestination::Global,
                    agent: "CodeX".to_string(),
                    enabled: true,
                })
                .expect("concurrent intent");
        })
    };
    barrier.wait();
    scanner.join().expect("scanner joins");
    writer.join().expect("writer joins");

    let catalog = skills.list_skills().expect("final catalog");
    assert_eq!(
        destination(
            custom_canonical(&catalog),
            &ProjectCustomSkillDestination::Global,
        )["CodeX"],
        PlacementCellRole::Target
    );
}

#[test]
#[serial]
fn successful_retry_resolves_matching_reconciliation_evidence() {
    let db = Arc::new(Database::open_in_memory().expect("open database"));
    let projects = ProjectService::new(db.clone());
    let skills = service(db.clone());
    let root = TempDir::new().expect("temp dir");
    isolate_home(&root);
    let repo = git_repo(&root, "source");
    projects.record_project(repo.clone()).expect("Project");
    write_skill(
        &Path::new(&repo).join("skills/retry"),
        "---\nname: retry\n---\n",
    );
    let rows = skills.scan_skills().expect("scan");
    let id = skill_id(custom_canonical(&rows)).to_string();
    {
        let conn = db.connection().expect("connection");
        conn.execute(
            "INSERT INTO skill_propagation_reconciliations (id, skill_id, destination_kind, target_project_id, intent_json, completed_steps_json, failed_compensations_json, observed_paths_json, created_at) VALUES ('e1', ?1, 'global', NULL, '{}', '[]', '[]', '{}', 1)",
            [&id],
        )
        .expect("seed evidence");
    }

    let result = skills
        .apply_project_custom_skill_intent(ProjectCustomSkillIntent::SetTargetEnabled {
            skill_id: id,
            destination: ProjectCustomSkillDestination::Global,
            enabled: false,
        })
        .expect("retry converges");
    assert!(!result.changed);
    let conn = db.connection().expect("connection");
    let resolved: Option<i64> = conn
        .query_row(
            "SELECT resolved_at FROM skill_propagation_reconciliations WHERE id = 'e1'",
            [],
            |row| row.get(0),
        )
        .expect("read evidence");
    assert!(resolved.is_some());
}

#[test]
#[serial]
fn withdrawal_preflights_every_placement_before_removing_any() {
    let db = Arc::new(Database::open_in_memory().expect("open database"));
    let projects = ProjectService::new(db.clone());
    let skills = service(db);
    let root = TempDir::new().expect("temp dir");
    let home = isolate_home(&root);
    let repo = git_repo(&root, "source");
    projects.record_project(repo.clone()).expect("Project");
    let source = Path::new(&repo).join("skills/preflight");
    write_skill(&source, "---\nname: preflight\n---\n");
    let rows = skills.scan_skills().expect("scan");
    let id = skill_id(custom_canonical(&rows)).to_string();
    set_agent(
        &skills,
        &id,
        ProjectCustomSkillDestination::Global,
        "Generic Agent",
        true,
    );
    set_agent(
        &skills,
        &id,
        ProjectCustomSkillDestination::Global,
        "CodeX",
        true,
    );
    let generic = home.join(".agents/skills/preflight");
    let codex = home.join(".codex/skills/preflight");
    #[cfg(unix)]
    fs::remove_file(&codex).expect("replace CodeX placement");
    #[cfg(windows)]
    fs::remove_dir(&codex).expect("replace CodeX placement");
    fs::create_dir_all(&codex).expect("create conflicting replacement");

    let error = skills
        .apply_project_custom_skill_intent(ProjectCustomSkillIntent::SetTargetEnabled {
            skill_id: id,
            destination: ProjectCustomSkillDestination::Global,
            enabled: false,
        })
        .expect_err("replacement conflicts with full withdrawal");
    assert!(matches!(error, AppError::Validation(_)));
    assert_link_points_to(&source, &generic);
    assert!(codex.is_dir(), "replacement was not removed");
}

fn database_failure_fixture() -> (
    Arc<Database>,
    SkillService,
    TempDir,
    String,
    std::path::PathBuf,
) {
    let db = Arc::new(Database::open_in_memory().expect("open database"));
    let projects = ProjectService::new(db.clone());
    let skills = service(db.clone());
    let root = TempDir::new().expect("temp dir");
    let home = isolate_home(&root);
    let repo = git_repo(&root, "source");
    projects.record_project(repo.clone()).expect("Project");
    write_skill(
        &Path::new(&repo).join("skills/failure"),
        "---\nname: failure\n---\n",
    );
    let rows = skills.scan_skills().expect("scan");
    let id = skill_id(custom_canonical(&rows)).to_string();
    (db, skills, root, id, home.join(".codex/skills/failure"))
}

#[test]
#[serial]
fn database_write_failure_rolls_back_rows_and_compensates_filesystem() {
    let (db, skills, _root, id, target) = database_failure_fixture();
    {
        let conn = db.connection().expect("connection");
        conn.execute_batch(
            "CREATE TRIGGER fail_skill_distribution_insert BEFORE INSERT ON skill_distributions WHEN NEW.role = 'target' BEGIN SELECT RAISE(FAIL, 'scripted database failure'); END;",
        )
        .expect("create failure trigger");
    }
    let error = skills
        .apply_project_custom_skill_intent(ProjectCustomSkillIntent::SetAgentPlacement {
            skill_id: id.clone(),
            destination: ProjectCustomSkillDestination::Global,
            agent: "CodeX".to_string(),
            enabled: true,
        })
        .expect_err("database failure");
    assert!(matches!(error, AppError::Database(_)));
    assert!(!target.exists(), "created Placement was compensated");
    assert!(
        !target.parent().expect("target parent").exists(),
        "empty parent directories created by the failed operation were compensated",
    );
    let evidence_count: i64 = db
        .connection()
        .expect("connection")
        .query_row(
            "SELECT COUNT(*) FROM skill_propagation_reconciliations",
            [],
            |row| row.get(0),
        )
        .expect("count evidence");
    assert_eq!(
        evidence_count, 0,
        "successful compensation writes no evidence"
    );
    let catalog = skills.list_skills().expect("catalog unchanged");
    assert_eq!(
        destination(
            custom_canonical(&catalog),
            &ProjectCustomSkillDestination::Global,
        )["CodeX"],
        PlacementCellRole::None
    );
}

#[test]
#[serial]
fn catalog_build_failure_rolls_back_transaction_and_compensates_filesystem() {
    let (db, skills, _root, id, target) = database_failure_fixture();
    {
        let conn = db.connection().expect("connection");
        conn.execute_batch(
            "CREATE TRIGGER corrupt_catalog_after_insert AFTER INSERT ON skill_distributions WHEN NEW.role = 'target' BEGIN UPDATE skills SET source_kind = 'agent', source_agent = NULL WHERE id = NEW.skill_id; END;",
        )
        .expect("create catalog failure trigger");
    }
    let error = skills
        .apply_project_custom_skill_intent(ProjectCustomSkillIntent::SetAgentPlacement {
            skill_id: id,
            destination: ProjectCustomSkillDestination::Global,
            agent: "CodeX".to_string(),
            enabled: true,
        })
        .expect_err("catalog build failure");
    assert!(matches!(error, AppError::Internal(_)));
    assert!(!target.exists(), "created Placement was compensated");
    assert!(matches!(
        custom_canonical(&skills.list_skills().expect("catalog after rollback")),
        SkillRow::ProjectCustomCanonical { .. }
    ));
}

#[test]
#[serial]
fn deferred_commit_failure_rolls_back_transaction_and_compensates_filesystem() {
    let (db, skills, _root, id, target) = database_failure_fixture();
    {
        let conn = db.connection().expect("connection");
        conn.pragma_update(None, "defer_foreign_keys", "ON")
            .expect("defer foreign keys");
        conn.execute_batch(
            "CREATE TRIGGER orphan_after_insert AFTER INSERT ON skill_distributions WHEN NEW.role = 'target' BEGIN INSERT INTO skill_project_distributions (skill_id, target_project_id, agent, role, target_path) VALUES (NEW.skill_id, 'missing-project', NEW.agent, 'none', NULL); END;",
        )
        .expect("create commit failure trigger");
    }
    let error = skills
        .apply_project_custom_skill_intent(ProjectCustomSkillIntent::SetAgentPlacement {
            skill_id: id,
            destination: ProjectCustomSkillDestination::Global,
            agent: "CodeX".to_string(),
            enabled: true,
        })
        .expect_err("commit failure");
    assert!(matches!(error, AppError::Database(_)));
    assert!(
        !target.exists(),
        "created Placement was compensated after commit failure"
    );
}

#[test]
fn canonical_display_path_helper_still_collapses_home() {
    let value = paths::collapse_home("/definitely/not/home");
    assert!(!value.is_empty());
}
