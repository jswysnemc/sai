use super::governance::AcpGovernance;
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::process::Child;
use tokio::sync::Mutex;

/// 一个受 sai 管理的终端。
struct Terminal {
    child: Option<Child>,
    /// 用户提交的原始命令，用于审计记录
    command: String,
    /// 累积输出；ACP 允许多次 `terminal/output` 读取同一终端
    output: Arc<Mutex<TerminalOutputCapture>>,
    /// 标准输出与错误的读取任务；进程退出后要等它们收完剩余数据
    readers: Vec<tokio::task::JoinHandle<()>>,
    exit_status: Option<i32>,
}

/// ACP 终端输出缓存。
#[derive(Default)]
struct TerminalOutputCapture {
    text: String,
    truncated: bool,
    byte_limit: Option<usize>,
}

impl TerminalOutputCapture {
    /// 追加一段终端输出，并按协议从开头截去超限内容。
    ///
    /// 参数:
    /// - `chunk`: 新收到的 UTF-8 输出
    ///
    /// 返回:
    /// - 无
    fn push(&mut self, chunk: &str) {
        self.text.push_str(chunk);
        let Some(limit) = self.byte_limit else {
            return;
        };
        if self.text.len() <= limit {
            return;
        }
        let mut start = self.text.len().saturating_sub(limit);
        while start < self.text.len() && !self.text.is_char_boundary(start) {
            start = start.saturating_add(1);
        }
        self.text.drain(..start);
        self.truncated = true;
    }
}

/// 外部内核可用的终端集合。
///
/// ACP 的终端是长驻有状态的（create → 多次 output → wait_for_exit → release），
/// 与 sai 一次性的 run_command 模型不同，因此这里自行维护终端表，
/// 但命令的沙箱构造与权限判定仍然复用 sai 的既有实现。
#[derive(Clone, Default)]
pub(crate) struct TerminalRegistry {
    terminals: Arc<Mutex<HashMap<String, Terminal>>>,
    next_id: Arc<AtomicU64>,
}

impl TerminalRegistry {
    /// 创建终端并启动命令。
    ///
    /// 参数:
    /// - `params`: `terminal/create` 参数
    /// - `governance`: 治理句柄
    /// - `events`: 事件发送端，用于呈现权限卡
    ///
    /// 返回:
    /// - 终端标识
    pub(crate) async fn create(
        &self,
        params: &Value,
        governance: &AcpGovernance,
        events: &crate::agent_engine::EventSender,
    ) -> Result<String> {
        let command = command_line(params)?;
        let original_command = command.clone();
        let cwd = params
            .get("cwd")
            .and_then(Value::as_str)
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| governance.workspace().to_path_buf());
        if !cwd.is_absolute() {
            anyhow::bail!("terminal/create cwd must be absolute");
        }
        // 1. 先过审核与权限：被拒或越界时直接失败，命令不会被启动
        //    审核针对用户看得懂的原始命令，而不是改写后的 rtk 形式
        let sandboxed = governance.authorize_command(&command, &cwd, events).await?;
        // 2. 再套用输出压缩，与自带 run_command 同一套判定
        let command = governance.apply_output_filter(&command);
        // 3. 复用 sai 的 shell 构造，沙箱与自带 run_command 完全一致；
        //    首选 shell 不存在时依次回退，与自带命令的行为保持一致
        let candidates = crate::tools::command::build_shell_commands(
            &command,
            governance.command_shell(),
            sandboxed,
        )?;
        let env = env_pairs(params);
        let mut spawned = None;
        let mut last_error = None;
        for (_, mut builder) in candidates {
            builder
                .current_dir(&cwd)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true);
            for (key, value) in &env {
                builder.env(key, value);
            }
            match builder.spawn() {
                Ok(child) => {
                    spawned = Some(child);
                    break;
                }
                // 该 shell 不在这台机器上，换下一个候选
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    last_error = Some(error);
                }
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("failed to start terminal command: {command}"))
                }
            }
        }
        let mut child = match spawned {
            Some(child) => child,
            None => {
                let detail = last_error
                    .map(|error| error.to_string())
                    .unwrap_or_else(|| "no shell was available".to_string());
                anyhow::bail!("failed to start terminal command: {command} ({detail})")
            }
        };
        let byte_limit = params
            .get("outputByteLimit")
            .and_then(Value::as_u64)
            .map(|value| usize::try_from(value).unwrap_or(usize::MAX));
        let output = Arc::new(Mutex::new(TerminalOutputCapture {
            byte_limit,
            ..Default::default()
        }));
        // 3. 标准输出与错误合并累积，读取端随时可取当前快照
        let mut readers = Vec::new();
        for stream in [
            child.stdout.take().map(StreamKind::Stdout),
            child.stderr.take().map(StreamKind::Stderr),
        ]
        .into_iter()
        .flatten()
        {
            let sink = Arc::clone(&output);
            let progress = events.clone();
            let label = command.clone();
            readers.push(tokio::spawn(async move {
                let mut buffer = [0u8; 4096];
                let mut reader = stream.into_reader();
                loop {
                    match reader.read(&mut buffer).await {
                        Ok(0) | Err(_) => break,
                        Ok(count) => {
                            let chunk = String::from_utf8_lossy(&buffer[..count]).to_string();
                            sink.lock().await.push(&chunk);
                            // 边收边推：长命令运行期间界面也能看到进度，
                            // 否则要等进程退出才一次性出现全部输出
                            let _ = progress.send(crate::agent::AgentEvent::ToolProgress {
                                name: label.clone(),
                                message: chunk,
                            });
                        }
                    }
                }
            }));
        }
        let id = format!("term-{}", self.next_id.fetch_add(1, Ordering::SeqCst));
        self.terminals.lock().await.insert(
            id.clone(),
            Terminal {
                child: Some(child),
                command: original_command,
                output,
                readers,
                exit_status: None,
            },
        );
        Ok(id)
    }

    /// 读取终端当前输出。
    ///
    /// 参数:
    /// - `terminal_id`: 终端标识
    ///
    /// 返回:
    /// - 输出与可选退出码
    pub(crate) async fn output(&self, terminal_id: &str) -> Result<Value> {
        let terminals = self.terminals.lock().await;
        let terminal = terminals
            .get(terminal_id)
            .with_context(|| format!("unknown terminal: {terminal_id}"))?;
        let output = terminal.output.lock().await;
        let text = output.text.clone();
        let truncated = output.truncated;
        Ok(json!({
            "output": text,
            "truncated": truncated,
            "exitStatus": terminal.exit_status.map(|code| json!({ "exitCode": code })),
        }))
    }

    /// 等待终端退出。
    ///
    /// 参数:
    /// - `terminal_id`: 终端标识
    /// - `governance`: 治理句柄，用于记录审计结果
    ///
    /// 返回:
    /// - 退出状态
    pub(crate) async fn wait_for_exit(
        &self,
        terminal_id: &str,
        governance: &AcpGovernance,
    ) -> Result<Value> {
        // 等待期间不持锁，否则同一终端的 output 调用会被阻塞
        let mut child = {
            let mut terminals = self.terminals.lock().await;
            let terminal = terminals
                .get_mut(terminal_id)
                .with_context(|| format!("unknown terminal: {terminal_id}"))?;
            match terminal.child.take() {
                Some(child) => child,
                // 已经等过一次，直接返回记录的退出码
                None => return Ok(json!({ "exitCode": terminal.exit_status.unwrap_or_default() })),
            }
        };
        let status = child.wait().await?;
        let code = status.code().unwrap_or(-1);
        // 进程退出不等于输出读完：管道里可能还有未读数据，
        // 必须等读取任务自然结束，否则 output 会返回不完整内容
        let readers = {
            let mut terminals = self.terminals.lock().await;
            terminals
                .get_mut(terminal_id)
                .map(|terminal| std::mem::take(&mut terminal.readers))
                .unwrap_or_default()
        };
        for reader in readers {
            let _ = reader.await;
        }
        let mut terminals = self.terminals.lock().await;
        if let Some(terminal) = terminals.get_mut(terminal_id) {
            terminal.exit_status = Some(code);
            let output = terminal.output.lock().await.text.clone();
            governance.record_command_result(&terminal.command, &output);
        }
        Ok(json!({ "exitCode": code }))
    }

    /// 结束终端进程。
    ///
    /// 参数:
    /// - `terminal_id`: 终端标识
    ///
    /// 返回:
    /// - 处理结果
    pub(crate) async fn kill(&self, terminal_id: &str) -> Result<()> {
        let mut terminals = self.terminals.lock().await;
        if let Some(terminal) = terminals.get_mut(terminal_id) {
            if let Some(child) = terminal.child.as_mut() {
                let _ = child.start_kill();
            }
        }
        Ok(())
    }

    /// 释放终端资源。
    ///
    /// 参数:
    /// - `terminal_id`: 终端标识
    ///
    /// 返回:
    /// - 处理结果
    pub(crate) async fn release(&self, terminal_id: &str) -> Result<()> {
        let mut terminals = self.terminals.lock().await;
        if let Some(mut terminal) = terminals.remove(terminal_id) {
            if let Some(child) = terminal.child.as_mut() {
                let _ = child.start_kill();
            }
            // 未等待退出就释放时读取任务仍在运行，主动中止避免任务泄漏
            for reader in terminal.readers.drain(..) {
                reader.abort();
            }
        }
        Ok(())
    }
}

/// 子进程输出流。
enum StreamKind {
    Stdout(tokio::process::ChildStdout),
    Stderr(tokio::process::ChildStderr),
}

impl StreamKind {
    /// 取出可异步读取的句柄。
    ///
    /// 返回:
    /// - 读取端
    fn into_reader(self) -> Box<dyn tokio::io::AsyncRead + Unpin + Send> {
        match self {
            Self::Stdout(stream) => Box::new(stream),
            Self::Stderr(stream) => Box::new(stream),
        }
    }
}

/// 从参数拼出完整命令行。
///
/// ACP 用 `command` 加 `args` 描述命令，sai 的沙箱按整条 shell 命令构造，
/// 因此在这里合并；每个参数按当前平台转义，避免被 shell 二次解析。
///
/// 参数:
/// - `params`: `terminal/create` 参数
///
/// 返回:
/// - 完整命令行
fn command_line(params: &Value) -> Result<String> {
    let command = params
        .get("command")
        .and_then(Value::as_str)
        .context("terminal/create requires a command")?;
    let mut line = command.to_string();
    if let Some(args) = params.get("args").and_then(Value::as_array) {
        for arg in args.iter().filter_map(Value::as_str) {
            line.push(' ');
            line.push_str(&quote_shell_arg(arg));
        }
    }
    Ok(line)
}

/// 将一个命令参数转义为 shell 中的单个参数。
///
/// 参数:
/// - `argument`: 原始参数
///
/// 返回:
/// - 可安全拼入命令行的参数文本
#[cfg(not(windows))]
fn quote_shell_arg(argument: &str) -> String {
    format!("'{}'", argument.replace('\'', "'\\''"))
}

/// 将一个命令参数转义为 Windows shell 中的单个参数。
///
/// 参数:
/// - `argument`: 原始参数
///
/// 返回:
/// - 可安全拼入命令行的参数文本
#[cfg(windows)]
fn quote_shell_arg(argument: &str) -> String {
    format!("\"{}\"", argument.replace('"', "\\\""))
}

/// 提取要注入子进程的环境变量。
///
/// 参数:
/// - `params`: `terminal/create` 参数
///
/// 返回:
/// - 键值对
fn env_pairs(params: &Value) -> Vec<(String, String)> {
    params
        .get("env")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let name = item.get("name").and_then(Value::as_str)?;
                    let value = item.get("value").and_then(Value::as_str)?;
                    Some((name.to_string(), value.to_string()))
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_command_with_args() {
        let line =
            command_line(&json!({ "command": "git", "args": ["status", "--short"] })).unwrap();
        #[cfg(not(windows))]
        assert_eq!(line, "git 'status' '--short'");
        #[cfg(windows)]
        assert_eq!(line, "git \"status\" \"--short\"");
    }

    /// 带空白的参数必须加引号，否则会被 shell 拆成多个参数。
    #[test]
    fn quotes_arguments_containing_whitespace() {
        let line =
            command_line(&json!({ "command": "git", "args": ["commit", "-m", "a b"] })).unwrap();
        #[cfg(not(windows))]
        assert_eq!(line, "git 'commit' '-m' 'a b'");
        #[cfg(windows)]
        assert_eq!(line, "git \"commit\" \"-m\" \"a b\"");
    }

    /// shell 元字符只能作为参数内容，不能变成额外命令。
    #[test]
    fn quotes_shell_metacharacters_in_arguments() {
        let line =
            command_line(&json!({ "command": "printf", "args": ["a; touch /tmp/x"] })).unwrap();
        assert!(line.contains("a; touch /tmp/x"));
        assert_ne!(line, "printf a; touch /tmp/x");
    }

    #[test]
    fn rejects_missing_command() {
        assert!(command_line(&json!({})).is_err());
    }

    #[test]
    fn reads_env_pairs() {
        let pairs = env_pairs(&json!({ "env": [{ "name": "A", "value": "1" }] }));
        assert_eq!(pairs, vec![("A".to_string(), "1".to_string())]);
    }

    /// 输出上限按字节执行，但不得截断 UTF-8 字符。
    #[test]
    fn terminal_output_limit_keeps_utf8_valid() {
        let mut capture = TerminalOutputCapture {
            byte_limit: Some(5),
            ..Default::default()
        };
        capture.push("ab中文");

        assert_eq!(capture.text, "文");
        assert!(capture.truncated);
    }

    /// 终端命令走的是 sai 自带 run_command 的同一套 shell 构造。
    ///
    /// 沙箱开启时首选程序必须是 bwrap——这条保证外部内核的命令
    /// 与 sai 自己执行命令受同样的隔离，而不是各建一套。
    #[cfg(target_os = "linux")]
    #[test]
    fn sandboxed_commands_go_through_bwrap() {
        let sandboxed = crate::tools::command::build_shell_commands("ls", "", true).unwrap();
        assert_eq!(sandboxed[0].0, "bwrap");

        let plain = crate::tools::command::build_shell_commands("ls", "", false).unwrap();
        assert_ne!(plain[0].0, "bwrap");
    }

    /// 候选列表必须非空，且在 Windows 上提供多个回退项。
    ///
    /// 首选 shell 不存在时要能换下一个，否则未装 pwsh 的机器上会出现
    /// 「sai 自带命令能跑、外部内核跑不了」的割裂。
    #[test]
    fn shell_candidates_are_never_empty() {
        let candidates = crate::tools::command::build_shell_commands("echo hi", "", false).unwrap();
        assert!(!candidates.is_empty());
        #[cfg(windows)]
        assert!(candidates.len() > 1, "Windows 需要提供回退候选");
    }

    /// 终端生命周期：创建后可读输出、可等待退出、释放后不再可见。
    #[tokio::test]
    async fn runs_a_command_and_reports_exit_code() {
        let dir = tempfile::tempdir().unwrap();
        let governance = AcpGovernance::new(
            dir.path().to_path_buf(),
            None,
            crate::config::AppConfig::default(),
            "test-session".to_string(),
            None,
            None,
        );
        let registry = TerminalRegistry::default();

        let (events, _rx) = tokio::sync::mpsc::unbounded_channel();
        let id = registry
            .create(
                &json!({ "command": "echo", "args": ["acp-terminal"] }),
                &governance,
                &events,
            )
            .await
            .unwrap();
        let exit = registry.wait_for_exit(&id, &governance).await.unwrap();
        assert_eq!(exit["exitCode"], 0);

        let output = registry.output(&id).await.unwrap();
        assert!(output["output"].as_str().unwrap().contains("acp-terminal"));

        registry.release(&id).await.unwrap();
        assert!(registry.output(&id).await.is_err());
    }

    /// 【ACP】【终端输出】验证退出后能读到全部输出，不受管道读取时序影响。
    ///
    /// child.wait() 只保证进程结束，管道里可能仍有未读数据；输出量较大时
    /// 更容易暴露该竞态。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[tokio::test]
    async fn output_is_complete_after_exit() {
        let dir = tempfile::tempdir().unwrap();
        let governance = AcpGovernance::new(
            dir.path().to_path_buf(),
            None,
            crate::config::AppConfig::default(),
            "output-session".to_string(),
            None,
            None,
        );
        let registry = TerminalRegistry::default();
        let (events, _rx) = tokio::sync::mpsc::unbounded_channel();

        // 输出多行以跨越单次读取缓冲，放大时序差异
        let id = registry
            .create(
                &json!({ "command": "seq", "args": ["1", "500"] }),
                &governance,
                &events,
            )
            .await
            .unwrap();
        registry.wait_for_exit(&id, &governance).await.unwrap();
        let output = registry.output(&id).await.unwrap();
        let text = output["output"].as_str().unwrap();

        assert!(text.contains('1'), "应包含首行");
        assert!(
            text.contains("500"),
            "退出后必须已读到末行: 实际 {} 字节",
            text.len()
        );
        registry.release(&id).await.unwrap();
    }

    /// 输出压缩关闭时命令原样执行。
    ///
    /// 顺序上审核先于改写：权限卡里要显示用户看得懂的原始命令，
    /// 而不是套了 rtk 前缀的形式。
    #[test]
    fn output_filter_is_off_by_default_in_tests() {
        let dir = tempfile::tempdir().unwrap();
        let governance = AcpGovernance::new(
            dir.path().to_path_buf(),
            None,
            crate::config::AppConfig::default(),
            "filter-session".to_string(),
            None,
            None,
        );

        // 测试环境探测不到 rtk，命令应原样返回
        assert_eq!(governance.apply_output_filter("git status"), "git status");
    }

    /// 未知终端标识必须报错，而不是返回空结果让 agent 以为成功。
    #[tokio::test]
    async fn unknown_terminal_is_reported_as_error() {
        let registry = TerminalRegistry::default();
        assert!(registry.output("missing").await.is_err());
    }
}
