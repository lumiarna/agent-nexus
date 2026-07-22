use std::{env, ffi::OsString, fs, path::Path, sync::Arc};

#[cfg(unix)]
use nexus_core::services::{
    prompts::{PromptService, SetPromptTargetInput},
    skills::SetSkillTargetInput,
    symlink::create_symlink_placement,
};
use nexus_core::{
    database::Database,
    services::{
        app_config::AppConfigService,
        paths,
        project_symlinks::ProjectSymlinkInventory,
        projects::ProjectService,
        skills::{ProjectCustomSkillDestination, ProjectCustomSkillIntent, SkillRow, SkillService},
    },
};
use serial_test::serial;
use tempfile::TempDir;

struct EnvVarGuard {
    key: &'static str,
    previous: Option<OsString>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: &Path) -> Self {
        let guard = Self {
            key,
            previous: env::var_os(key),
        };
        env::set_var(key, value);
        guard
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        match self.previous.take() {
            Some(value) => env::set_var(self.key, value),
            None => env::remove_var(self.key),
        }
    }
}

fn git_repo(parent: &TempDir, name: &str) -> String {
    let path = parent.path().join(name);
    fs::create_dir_all(path.join(".git")).expect("create test git repo");
    path.to_string_lossy().into_owned()
}

fn display_path(path: &Path) -> String {
    let path = paths::path_to_string(path, "path").expect("display path");
    paths::collapse_home(&path)
}

fn create_directory_link(source: &Path, target: &Path) {
    #[cfg(windows)]
    nexus_core::services::symlink::create_junction_placement(source, target)
        .expect("create junction link");
    #[cfg(not(windows))]
    nexus_core::services::symlink::create_symlink_placement(source, target)
        .expect("create symlink link");
}

fn remove_directory_link(path: &Path) {
    #[cfg(windows)]
    fs::remove_dir(path).expect("remove junction link");
    #[cfg(not(windows))]
    fs::remove_file(path).expect("remove symlink link");
}

fn write_skill(dir: &Path) {
    fs::create_dir_all(dir).expect("create skill dir");
    fs::write(
        dir.join("SKILL.md"),
        "---\nname: shared-skill\ndescription: Shared project skill\n---\n",
    )
    .expect("write project skill");
}

#[test]
#[serial]
fn lists_and_deletes_registered_project_symlinks() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let projects = ProjectService::new(db.clone());
    let inventory = ProjectSymlinkInventory::new(db);
    let root = TempDir::new().expect("create temp dir");
    let source_repo = git_repo(&root, "source-project");
    let target_repo = git_repo(&root, "target-project");
    let source_dir = Path::new(&source_repo).join("shared");
    let target_link = Path::new(&target_repo).join("shared");
    fs::create_dir_all(&source_dir).expect("create source dir");
    create_directory_link(&source_dir, &target_link);

    projects
        .record_project(source_repo)
        .expect("record source project");
    projects
        .record_project(target_repo.clone())
        .expect("record target project");

    let links = inventory
        .list_project_symlinks()
        .expect("list project symlinks");

    assert_eq!(links.len(), 1);
    assert_eq!(
        links[0].target_path,
        display_path(
            &fs::canonicalize(Path::new(&target_repo).parent().unwrap())
                .unwrap()
                .join("target-project/shared")
        )
    );

    inventory
        .delete_project_symlink(links[0].target_path.clone())
        .expect("delete project symlink");

    assert!(inventory
        .list_project_symlinks()
        .expect("list project symlinks")
        .is_empty());
    assert!(!target_link.exists());
}

#[test]
#[serial]
fn lists_project_symlinks_by_project_display_order() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let projects = ProjectService::new(db.clone());
    let inventory = ProjectSymlinkInventory::new(db);
    let root = TempDir::new().expect("create temp dir");
    let source_alpha_repo = git_repo(&root, "source-alpha");
    let source_beta_repo = git_repo(&root, "source-beta");
    let target_repo = git_repo(&root, "target-project");
    let source_alpha_dir = Path::new(&source_alpha_repo).join("shared");
    let source_beta_dir = Path::new(&source_beta_repo).join("shared");
    let target_alpha_link = Path::new(&target_repo).join("from-alpha");
    let target_beta_link = Path::new(&target_repo).join("from-beta");
    fs::create_dir_all(&source_alpha_dir).expect("create alpha source dir");
    fs::create_dir_all(&source_beta_dir).expect("create beta source dir");
    create_directory_link(&source_alpha_dir, &target_alpha_link);
    create_directory_link(&source_beta_dir, &target_beta_link);

    let source_alpha = projects
        .record_project(source_alpha_repo)
        .expect("record alpha source project");
    let source_beta = projects
        .record_project(source_beta_repo)
        .expect("record beta source project");
    let target = projects
        .record_project(target_repo)
        .expect("record target project");
    projects
        .reorder_projects(vec![
            source_beta.id.clone(),
            source_alpha.id.clone(),
            target.id.clone(),
        ])
        .expect("reorder projects");

    let links = inventory
        .list_project_symlinks()
        .expect("list project symlinks");

    assert_eq!(links.len(), 2);
    assert_eq!(
        links
            .iter()
            .map(|link| link.source_project_name.as_deref())
            .collect::<Vec<_>>(),
        vec![Some("source-beta"), Some("source-alpha")]
    );
}

#[cfg(unix)]
#[test]
#[serial]
fn skips_project_skill_and_prompt_distribution_links() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let projects = ProjectService::new(db.clone());
    let skills = SkillService::new(db.clone(), AppConfigService::new(db.clone()));
    let prompts = PromptService::new(db.clone());
    let inventory = ProjectSymlinkInventory::new(db);
    let root = TempDir::new().expect("create temp dir");
    let repo = git_repo(&root, "distribution-project");
    let project = projects
        .record_project(repo.clone())
        .expect("record project");
    let repo = Path::new(&repo);

    write_skill(&repo.join(".github/skills/shared-skill"));
    fs::write(repo.join("AGENTS.md"), "# Project instructions\n").expect("write project prompt");

    let skill = skills
        .scan_skills()
        .expect("scan project skill")
        .into_iter()
        .find(|skill| matches!(
            skill,
            SkillRow::AgentCanonical {
                context: nexus_core::services::skills::SkillContext::Project { project: row_project },
                ..
            } if row_project.id == project.id
        ))
        .expect("find project skill");
    skills
        .set_skill_target(SetSkillTargetInput {
            skill_id: skill.skill().skill_id.clone(),
            agent: "CodeX".to_string(),
            enabled: true,
        })
        .expect("enable project skill distribution");

    let prompt = prompts
        .scan_prompts()
        .expect("scan project prompt")
        .into_iter()
        .find(|prompt| prompt.project_id.as_deref() == Some(project.id.as_str()))
        .expect("find project prompt");
    prompts
        .set_prompt_target(SetPromptTargetInput {
            prompt_id: prompt.id,
            agent: "Claude Code".to_string(),
            enabled: true,
        })
        .expect("enable project prompt distribution");

    let links = inventory
        .list_project_symlinks()
        .expect("list project symlinks");

    assert!(
        links.is_empty(),
        "project Distribution placements should be hidden, got {links:?}"
    );

    let prompt_target = repo.join("CLAUDE.md");
    fs::remove_file(&prompt_target).expect("remove managed prompt link");
    let replacement_source = root.path().join("replacement.md");
    fs::write(&replacement_source, "# Replacement\n").expect("write replacement prompt");
    create_symlink_placement(&replacement_source, &prompt_target)
        .expect("replace managed target with an unrelated link");

    let links = inventory
        .list_project_symlinks()
        .expect("list replaced project symlink");
    assert_eq!(links.len(), 1, "unrelated replacement should be listed");
    assert!(Path::new(&links[0].target_path).ends_with("CLAUDE.md"));
}

#[test]
#[serial]
fn skips_project_custom_skill_propagation_placements_but_keeps_replacements_and_unrelated_links() {
    let db = Arc::new(Database::open_in_memory().expect("open in-memory database"));
    let projects = ProjectService::new(db.clone());
    let skills = SkillService::new(db.clone(), AppConfigService::new(db.clone()));
    let inventory = ProjectSymlinkInventory::new(db);
    let root = TempDir::new().expect("create temp dir");
    let home = root.path().join("home");
    fs::create_dir_all(&home).expect("create isolated home");
    let _home_guard = EnvVarGuard::set("HOME", &home);
    let _userprofile_guard = EnvVarGuard::set("USERPROFILE", &home);

    let source_repo = git_repo(&root, "source-project");
    let target_repo = git_repo(&root, "target-project");
    let source_project = projects
        .record_project(source_repo.clone())
        .expect("record source project");
    let target_project = projects
        .record_project(target_repo.clone())
        .expect("record target project");
    let source_root = Path::new(&source_repo);
    let target_root = Path::new(&target_repo);
    let custom_dir = source_root.join("skills/release-notes");
    write_skill(&custom_dir);

    let skill_id = skills
        .scan_skills()
        .expect("scan project custom skill")
        .into_iter()
        .find(|skill| matches!(skill, SkillRow::ProjectCustomCanonical { .. }))
        .expect("find project custom skill")
        .skill()
        .skill_id
        .clone();

    // Propagate to both the source/current Project and another Project, then
    // fan each placement out to a second Agent through the single typed intent.
    for (project_id, agent) in [
        (source_project.id.clone(), "Claude Code"),
        (source_project.id.clone(), "CodeX"),
        (target_project.id.clone(), "Generic Agent"),
        (target_project.id.clone(), "Pi"),
    ] {
        skills
            .apply_project_custom_skill_intent(ProjectCustomSkillIntent::SetAgentPlacement {
                skill_id: skill_id.clone(),
                destination: ProjectCustomSkillDestination::Project { project_id },
                agent: agent.to_string(),
                enabled: true,
            })
            .expect("set Project custom Skill placement");
    }

    let unrelated_source = root.path().join("unrelated-source");
    fs::create_dir_all(&unrelated_source).expect("create unrelated source");
    let unrelated_link = target_root.join("manual-link");
    create_directory_link(&unrelated_source, &unrelated_link);

    let managed_targets = [
        source_root.join(".claude/skills/release-notes"),
        source_root.join(".codex/skills/release-notes"),
        target_root.join(".agents/skills/release-notes"),
        target_root.join(".pi/skills/release-notes"),
    ];
    let links = inventory
        .list_project_symlinks()
        .expect("list project symlinks");
    assert_eq!(links.len(), 1, "only the unrelated link should be listed");
    assert!(Path::new(&links[0].target_path).ends_with("manual-link"));
    for target in &managed_targets {
        assert!(
            !links
                .iter()
                .any(|link| Path::new(&link.target_path) == target),
            "managed Project Skill placement should be hidden: {}",
            target.display()
        );
    }

    // Keep the distribution row but replace one managed link with a link to an
    // unrelated source. Matching the target path alone must not hide it.
    let replaced_target = &managed_targets[1];
    remove_directory_link(replaced_target);
    let replacement_source = root.path().join("replacement-skill");
    fs::create_dir_all(&replacement_source).expect("create replacement source");
    create_directory_link(&replacement_source, replaced_target);

    let links = inventory
        .list_project_symlinks()
        .expect("list replaced project symlinks");
    assert_eq!(
        links.len(),
        2,
        "replacement and unrelated links should show"
    );
    assert!(
        links
            .iter()
            .any(|link| Path::new(&link.target_path).ends_with(".codex/skills/release-notes")),
        "replacement target should be listed: {links:?}"
    );
    assert!(
        links
            .iter()
            .any(|link| Path::new(&link.target_path).ends_with("manual-link")),
        "unrelated target should be listed: {links:?}"
    );
    assert!(
        !nexus_core::services::distribution::placement_points_to(
            replaced_target,
            &root.path().join("deleted-canonical-source"),
        )
        .expect("stale canonical source should be treated as an unmanaged link"),
        "a removed canonical source must not keep a replacement hidden"
    );
}
