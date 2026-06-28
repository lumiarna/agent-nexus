use std::{fs, path::Path};

use crate::{
    error::{AppError, AppResult},
    services::paths::resolve_local_path,
};

use super::{
    file_state::{FileState, FileStateMap},
    Task,
};

pub(super) struct LocalCopy;

impl LocalCopy {
    pub(super) fn run(task: &Task, file_states: &FileStateMap) -> AppResult<()> {
        let source = resolve_local_path(&task.source)?;
        let target = resolve_local_path(&task.target)?;

        if !source.exists() {
            return Err(AppError::Validation(format!(
                "local source does not exist: {}",
                task.source
            )));
        }

        if fs::metadata(&source)?.is_file() {
            let rel_path = required_file_name(&source)?;
            if !FileState::should_skip(&source, &rel_path, file_states)? {
                copy_file(&source, &target)?;
            }
        } else {
            copy_directory(&source, &target, file_states)?;
        }

        Ok(())
    }
}

fn copy_directory(source: &Path, target: &Path, file_states: &FileStateMap) -> AppResult<()> {
    let effective_target = if target.exists() && target.is_dir() {
        let name = required_file_name(source)?;
        target.join(name)
    } else {
        target.to_path_buf()
    };
    copy_directory_tree(source, &effective_target, file_states, source)
}

fn copy_file(source: &Path, target: &Path) -> AppResult<()> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(source, target)?;
    Ok(())
}

fn copy_directory_tree(
    source: &Path,
    target: &Path,
    file_states: &FileStateMap,
    source_root: &Path,
) -> AppResult<()> {
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();
        let dest = target.join(entry.file_name());
        let metadata = fs::metadata(&path)?;
        if metadata.is_dir() {
            copy_directory_tree(&path, &dest, file_states, source_root)?;
        } else {
            let rel_path = path
                .strip_prefix(source_root)
                .map_err(|_| {
                    AppError::Internal("failed to compute relative path for copy".to_string())
                })?
                .to_string_lossy()
                .replace('\\', "/");
            if !FileState::should_skip(&path, &rel_path, file_states)? {
                copy_file(&path, &dest)?;
            }
        }
    }
    Ok(())
}

fn required_file_name(path: &Path) -> AppResult<String> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(ToOwned::to_owned)
        .ok_or_else(|| AppError::Validation("path file name must be valid UTF-8".to_string()))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn directory_copy_embeds_source_when_target_directory_exists() {
        let root = TempDir::new().expect("create temp dir");
        let source = root.path().join("source");
        fs::create_dir_all(source.join("sub")).expect("create source dirs");
        fs::write(source.join("a.txt"), "alpha").expect("write a");
        fs::write(source.join("sub").join("b.txt"), "beta").expect("write b");

        let target = root.path().join("target");
        fs::create_dir_all(&target).expect("create target dir");
        fs::write(target.join("c.txt"), "gamma").expect("write sibling");
        let embedded = target.join("source");
        fs::create_dir_all(&embedded).expect("create embedded dir");
        fs::write(embedded.join("stale.txt"), "stale").expect("write stale file");

        let task = task(&source, &target);

        LocalCopy::run(&task, &HashMap::new()).expect("run local copy");

        assert_eq!(
            fs::read_to_string(target.join("source").join("a.txt")).expect("read copied a"),
            "alpha"
        );
        assert_eq!(
            fs::read_to_string(target.join("source").join("sub").join("b.txt"))
                .expect("read copied b"),
            "beta"
        );
        assert_eq!(
            fs::read_to_string(target.join("c.txt")).expect("read sibling"),
            "gamma"
        );
        assert!(
            target.join("source").join("stale.txt").exists(),
            "incremental copy preserves stale target files"
        );
    }

    #[test]
    fn missing_source_returns_validation_error_without_creating_target() {
        let root = TempDir::new().expect("create temp dir");
        let source = root.path().join("missing.txt");
        let target = root.path().join("target.txt");
        let task = task(&source, &target);

        let error = LocalCopy::run(&task, &HashMap::new()).expect_err("missing source should fail");

        match error {
            AppError::Validation(message) => {
                assert!(message.contains("source does not exist"));
            }
            other => panic!("expected validation error, got {other:?}"),
        }
        assert!(!target.exists(), "target should not be created");
    }

    fn task(source: &Path, target: &Path) -> Task {
        Task {
            id: "task-1".to_string(),
            direction: "Distribution".to_string(),
            action: "Copy".to_string(),
            source_type: "Local".to_string(),
            source: source.to_string_lossy().into_owned(),
            target_type: "Local".to_string(),
            target: target.to_string_lossy().into_owned(),
            schedule: "manual".to_string(),
            last_run_at: None,
            status: "never".to_string(),
            link_state: "none".to_string(),
        }
    }
}
