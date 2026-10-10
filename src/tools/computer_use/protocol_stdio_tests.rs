//! 通过隔离的 Python 标准库夹具验证 stdio 子进程所有权和取消清理。

use super::driver::CuaDriver;
use super::session::{ComputerSession, DesktopCoordinator};
use super::test_support::{input_args, observe_args, state};
use crate::config::{ComputerUseConfig, McpServerConfig};
use crate::mcp::McpConnection;
use crate::tools::ToolRegistry;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

/// 【桌面测试】【进程记录】读取夹具写出的请求，忽略尚未完成写入的末行。
/// @param path 为临时记录文件
/// @returns 完整请求记录
fn records(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

/// 【桌面测试】【进程等待】有界等待夹具条件，避免测试依赖任意固定延时。
/// @param predicate 为完成条件
/// @returns 条件成立时结束；超时则测试失败
async fn wait_for(predicate: impl Fn() -> bool) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while !predicate() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("native fixture condition timed out");
}

/// 【桌面测试】【进程检查】只查询本测试启动的进程是否仍存在，不发送信号。
/// @param pid 为夹具进程号
/// @returns 进程仍存在时为 true
fn is_alive(pid: i32) -> bool {
    // 1. 【桌面测试】【只读探测】信号 0 只检查存在性，不改变进程状态
    unsafe { libc::kill(pid, 0) == 0 }
}

/// 【桌面测试】【stdio 取消】取消原生调用会回收子进程，旧输入不能在新进程重放。
#[tokio::test]
async fn owned_stdio_driver_is_killed_on_cancel_and_reset() {
    let root = tempfile::tempdir().unwrap();
    let log = root.path().join("requests.jsonl");
    let home = root.path().join("isolated home");
    std::fs::create_dir(&home).unwrap();
    let config = McpServerConfig {
        id: "computer-use-stdio-fixture".into(),
        enabled: true,
        transport: "stdio".into(),
        command: "python3".into(),
        args: vec![
            "-I".into(),
            "-u".into(),
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/tools/computer_use/fixtures/driver.py"
            )
            .into(),
        ],
        env: HashMap::from([
            ("HOME".into(), home.to_string_lossy().into_owned()),
            (
                "XDG_CONFIG_HOME".into(),
                home.to_string_lossy().into_owned(),
            ),
            ("XDG_DATA_HOME".into(), home.to_string_lossy().into_owned()),
            (
                "SAI_FIXTURE_RECORDS".into(),
                log.to_string_lossy().into_owned(),
            ),
            ("SAI_FIXTURE_STATE".into(), state(true).to_string()),
        ]),
        cwd: Some(home.to_string_lossy().into_owned()),
        url: None,
        message_url: None,
        headers: HashMap::new(),
        timeout_ms: Some(5000),
    };
    let session = ComputerSession::new(
        Arc::new(CuaDriver::with_connection(McpConnection::new(config))),
        Arc::new(DesktopCoordinator::default()),
    );
    let mut registry = ToolRegistry::new();
    super::register_session(
        &mut registry,
        &ComputerUseConfig {
            enabled: true,
            ..Default::default()
        },
        session,
    );
    let observed = registry
        .call(super::TOOL_NAME, &observe_args().to_string())
        .await
        .unwrap();
    let first_pid = records(&log)[0]["pid"].as_i64().unwrap() as i32;
    let mut click = input_args(&observed, "click");
    click["element_index"] = json!("g1:0");
    let running_registry = registry.clone();
    let running_args = click.to_string();
    let pending =
        tokio::spawn(async move { running_registry.call(super::TOOL_NAME, &running_args).await });
    wait_for(|| {
        records(&log)
            .iter()
            .any(|row| row["request"]["params"]["name"] == "click")
    })
    .await;
    pending.abort();
    assert!(pending.await.unwrap_err().is_cancelled());
    wait_for(|| !is_alive(first_pid)).await;
    assert!(registry
        .call(super::TOOL_NAME, &click.to_string())
        .await
        .is_err());

    registry
        .call(super::TOOL_NAME, &observe_args().to_string())
        .await
        .unwrap();
    let calls = records(&log);
    assert_eq!(
        calls
            .iter()
            .filter(|row| row["request"]["method"] == "initialize")
            .count(),
        2
    );
    assert_eq!(
        calls
            .iter()
            .filter(|row| row["request"]["params"]["name"] == "click")
            .count(),
        1
    );
    let second_pid = calls.last().unwrap()["pid"].as_i64().unwrap() as i32;
    registry
        .call(super::TOOL_NAME, r#"{"action":"reset"}"#)
        .await
        .unwrap();
    wait_for(|| !is_alive(second_pid)).await;
}
