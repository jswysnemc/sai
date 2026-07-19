use super::*;

#[tokio::test]
async fn clones_real_repository_into_selected_parent() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    let parent = temp.path().join("clones");
    tokio::fs::create_dir(&source).await.unwrap();
    tokio::fs::create_dir(&parent).await.unwrap();
    git_success(&source, &["init", "-b", "main"]).await.unwrap();
    git_success(&source, &["config", "user.name", "Sai Test"])
        .await
        .unwrap();
    git_success(&source, &["config", "user.email", "sai@example.com"])
        .await
        .unwrap();
    tokio::fs::write(source.join("tracked.txt"), "initial\n")
        .await
        .unwrap();
    git_success(&source, &["add", "tracked.txt"]).await.unwrap();
    git_success(&source, &["commit", "-m", "initial"])
        .await
        .unwrap();

    let response = git_clone(&parent, source.to_str().unwrap(), Some("copy"))
        .await
        .unwrap();

    assert!(response.ok, "{}", response.stderr);
    assert!(response.state.has_commits);
    assert_eq!(response.state.head, "main");
    assert!(parent.join("copy/tracked.txt").exists());
}

#[tokio::test]
async fn rejects_clone_directory_escape() {
    let temp = tempfile::tempdir().unwrap();
    let error = git_clone(temp.path(), "https://example.com/repo.git", Some("../repo"))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("single folder name"));
}
