use super::*;

fn temp_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "sai-subagent-worktree-{label}-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ))
}

fn git(cwd: &Path, args: &[&str]) -> Result<String, String> {
    run_git_owned(cwd, args.iter().map(|arg| (*arg).to_string()).collect())
}

fn init_repo(root: &Path) -> Result<(), String> {
    fs::create_dir_all(root).map_err(|err| format!("failed to create repo: {err}"))?;
    git(root, &["init"])?;
    git(root, &["config", "user.email", "sai-test@example.com"])?;
    git(root, &["config", "user.name", "Sai Test"])?;
    git(root, &["config", "core.autocrlf", "false"])?;
    fs::write(root.join("README.md"), "base\n")
        .map_err(|err| format!("failed to write README: {err}"))?;
    git(root, &["add", "README.md"])?;
    git(root, &["commit", "-m", "init"])?;
    Ok(())
}

#[test]
fn creates_applies_and_cleans_worktree() -> Result<(), String> {
    let root = temp_root("create-apply");
    let repo = root.join("repo");
    init_repo(&repo)?;

    let worktree = try_create(&repo, "edit")
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "expected worktree".to_string())?;
    assert!(worktree.worktree_root.exists());
    assert!(is_sai_subagent_worktree(&worktree.worktree_root));

    fs::create_dir_all(worktree.workdir.join("test"))
        .map_err(|err| format!("failed to create test dir: {err}"))?;
    fs::write(
        worktree.workdir.join("test/agent.md"),
        "# Agent CRUD Test\n\n- status: done\n",
    )
    .map_err(|err| format!("failed to write worktree file: {err}"))?;

    let apply_result = apply(&worktree).map_err(|err| err.to_string())?;
    assert!(apply_result.applied);
    assert_eq!(
        fs::read_to_string(repo.join("test/agent.md"))
            .map_err(|err| format!("failed to read parent file: {err}"))?,
        "# Agent CRUD Test\n\n- status: done\n"
    );

    let cleanup_result = cleanup(&worktree);
    assert!(cleanup_result.removed);
    assert!(!worktree.worktree_root.exists());

    let _ = fs::remove_dir_all(root);
    Ok(())
}

#[test]
fn skips_non_git_directory() {
    let temp = tempfile::tempdir().unwrap();
    let result = try_create(temp.path(), "agent").unwrap();
    assert!(result.is_none());
}
