use crate::sandbox::{FileAccess, SandboxPolicy};
use anyhow::{Context, Result};
use std::path::Path;

/// 【Windows沙箱】【原生回归】执行实际受限 Shell，验证读取、写入、保护路径与越界拒绝。
/// 参数：无；返回隔离断言结果，不因运行环境不支持而跳过。
pub(crate) fn run() -> Result<()> {
    let root = tempfile::tempdir()?;
    let workspace = root.path().join("workspace");
    let outside = root.path().join("outside");
    let temp = root.path().join("scratch");
    for path in [&workspace, &outside, &temp] {
        std::fs::create_dir(path)?;
    }
    std::fs::write(workspace.join("read.txt"), "visible")?;
    std::fs::write(outside.join("secret.txt"), "secret")?;
    let protected = workspace.join("protected");
    std::fs::create_dir(&protected)?;
    std::fs::write(protected.join("rules.txt"), "original")?;
    let hidden = workspace.join("hidden.txt");
    std::fs::write(&hidden, "private")?;
    let mut policy = SandboxPolicy {
        access: FileAccess::ReadOnly,
        cwd: workspace.clone(),
        writable_roots: vec![temp],
        write_protected: vec![protected.clone()],
        deny_read: vec![hidden.clone()],
        network: false,
        allowed_domains: Vec::new(),
        temp_dir: None,
        env: Vec::new(),
        scrubbed_env: Vec::new(),
    };
    let shell =
        Path::new(&std::env::var_os("SystemRoot").context("SystemRoot")?).join("System32/cmd.exe");
    let execute = |policy: &SandboxPolicy, script: &str| -> Result<std::process::Output> {
        crate::sandbox::launch(policy, &shell.to_string_lossy(), script)?
            .into_std()
            .output()
            .context("sandbox command")
    };
    let read = execute(&policy, "type read.txt")?;
    anyhow::ensure!(
        read.status.success() && String::from_utf8_lossy(&read.stdout).contains("visible"),
        "read failed: {}",
        String::from_utf8_lossy(&read.stderr)
    );
    let write = execute(&policy, "echo denied> blocked.txt")?;
    anyhow::ensure!(
        !write.status.success() && !workspace.join("blocked.txt").exists(),
        "readonly write escaped"
    );
    let secret = execute(
        &policy,
        &format!("type \"{}\"", outside.join("secret.txt").display()),
    )?;
    anyhow::ensure!(
        !secret.status.success() && !String::from_utf8_lossy(&secret.stdout).contains("secret"),
        "outside read escaped"
    );
    let hidden_read = execute(&policy, "type hidden.txt")?;
    anyhow::ensure!(!hidden_read.status.success(), "hidden file readable");
    policy.access = FileAccess::WorkspaceWrite;
    policy.writable_roots.push(workspace.clone());
    let write = execute(&policy, "echo allowed> allowed.txt")?;
    anyhow::ensure!(
        write.status.success() && workspace.join("allowed.txt").exists(),
        "workspace write failed: {}",
        String::from_utf8_lossy(&write.stderr)
    );
    let protection = execute(&policy, "echo changed> protected\\rules.txt")?;
    anyhow::ensure!(
        !protection.status.success()
            && std::fs::read_to_string(protected.join("rules.txt"))? == "original",
        "protected file changed"
    );
    let outside_write = execute(
        &policy,
        &format!(
            "echo escaped> \"{}\"",
            outside.join("escaped.txt").display()
        ),
    )?;
    anyhow::ensure!(
        !outside_write.status.success() && !outside.join("escaped.txt").exists(),
        "outside write escaped"
    );
    println!("Windows AppContainer read/write isolation regressions passed");
    Ok(())
}
