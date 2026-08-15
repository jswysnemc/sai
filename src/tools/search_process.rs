use anyhow::Result;
use std::io::ErrorKind;
use std::process::{Output, Stdio};
use tokio::process::Command;

pub(super) const SEARCH_TIMEOUT_SECONDS: u64 = 30;

/// 搜索命令的执行结果。
pub(super) enum SearchRun {
    /// 命令正常结束
    Finished(Output),
    /// 超时终止，携带已经收到的部分输出
    TimedOut(Vec<u8>),
    /// 程序不存在，触发内置回退
    Missing,
}

/// 执行搜索命令，程序不存在时返回 Missing 以触发内置回退。
///
/// 超时不再整体失败：ripgrep 按行输出，先到的匹配同样有价值。
/// 此处终止子进程并交回已收到的部分输出，由调用方标注截断。
///
/// 参数:
/// - `command`: 已完成参数配置的搜索命令
///
/// 返回:
/// - 命令执行结果
pub(super) async fn run_search_command(command: Command) -> Result<SearchRun> {
    run_search_command_with_timeout(command, SEARCH_TIMEOUT_SECONDS).await
}

/// 按指定超时执行搜索命令。
///
/// 参数:
/// - `command`: 已完成参数配置的搜索命令
/// - `timeout_seconds`: 超时秒数
///
/// 返回:
/// - 命令执行结果
pub(super) async fn run_search_command_with_timeout(
    mut command: Command,
    timeout_seconds: u64,
) -> Result<SearchRun> {
    // 1. 改用 spawn 以便超时后能拿到子进程句柄并主动终止，
    //    output() 超时只会丢弃 future，子进程会继续跑成孤儿
    let mut child = match command.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn() {
        Ok(child) => child,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(SearchRun::Missing),
        Err(err) => return Err(err.into()),
    };
    let mut stdout = child.stdout.take().expect("stdout is piped");
    let mut stderr = child.stderr.take().expect("stderr is piped");
    let mut collected = Vec::new();
    let mut collected_err = Vec::new();
    // 2. 两个管道并发读取：只读 stdout 会在 stderr 写满管道缓冲时双双阻塞。
    //    边读边存，超时时这些已读内容就是可返回的部分结果
    let read = async {
        tokio::try_join!(
            tokio::io::AsyncReadExt::read_to_end(&mut stdout, &mut collected),
            tokio::io::AsyncReadExt::read_to_end(&mut stderr, &mut collected_err),
        )?;
        child.wait().await
    };
    match tokio::time::timeout(std::time::Duration::from_secs(timeout_seconds), read).await {
        Ok(Ok(status)) => Ok(SearchRun::Finished(Output {
            status,
            stdout: collected,
            stderr: collected_err,
        })),
        Ok(Err(err)) => Err(err.into()),
        Err(_) => {
            // 3. 终止子进程，避免留下继续扫描磁盘的孤儿 ripgrep
            let _ = child.kill().await;
            Ok(SearchRun::TimedOut(collected))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证超时确实终止子进程并交回已读到的部分输出。
    ///
    /// 只测格式化函数无法覆盖 spawn / 读取 / kill 这条真实路径。
    #[tokio::test]
    async fn timeout_kills_child_and_returns_partial_output() {
        // 直接把脚本作为 sh 的参数传入，不落地成可执行文件：
        // 并发测试里同时发生的 fork 会继承尚未关闭的写句柄，
        // 内核据此在 exec 时报 ETXTBSY，导致本用例偶发失败。
        //
        // sleep 的输出必须重定向掉：kill 只终止直接子进程，孙进程若继承了
        // 管道写端，读端就等不到 EOF。Windows 上负责阻塞读取的线程因此退不出，
        // runtime 析构要一直等到 sleep 自己结束。MSYS2 的 sh 没有真正的 exec
        // 语义，靠 exec 合并进程在那里不成立，切断继承才是可移植的做法。
        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg("echo 'src/a.rs:1:early hit'; exec sleep 30 >/dev/null 2>&1")
            .stdin(Stdio::null());

        let started = std::time::Instant::now();
        let run = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            run_search_command_with_timeout(command, 1),
        )
        .await
        .expect("run_search_command must return once its own timeout fires")
        .unwrap();

        assert!(started.elapsed() < std::time::Duration::from_secs(5));
        match run {
            SearchRun::TimedOut(partial) => {
                let text = String::from_utf8_lossy(&partial);
                assert!(text.contains("early hit"), "应保留超时前已读到的匹配");
            }
            _ => panic!("慢命令应触发超时分支"),
        }
    }

    /// 验证程序不存在时返回 Missing 以触发内置回退。
    #[tokio::test]
    async fn missing_program_triggers_native_fallback() {
        let command = Command::new("sai-definitely-not-a-real-program");

        let run = run_search_command(command).await.unwrap();

        assert!(matches!(run, SearchRun::Missing));
    }
}
