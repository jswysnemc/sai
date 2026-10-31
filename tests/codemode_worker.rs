use sai_codemode::catalog::CatalogEntry;
use sai_codemode::protocol::{self, ExecutionRequest, ToolReply, WorkerEvent};
use serde_json::json;
use std::io::BufReader;
use std::process::{Command, Stdio};

/// 【CodeMode】【进程测试】用 code 启动实际 Sai 工作进程，返回进程及两个通信端。
fn worker(
    code: &str,
) -> (
    std::process::Child,
    std::process::ChildStdin,
    BufReader<std::process::ChildStdout>,
) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_sai"))
        .arg(protocol::WORKER_ARG)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let output = BufReader::new(child.stdout.take().unwrap());
    protocol::write_frame(
        &mut input,
        &ExecutionRequest {
            code: code.into(),
            timeout_ms: 1000,
            catalog: vec![CatalogEntry::new(
                "echo".into(),
                "Echo a value".into(),
                json!({"type":"object"}),
            )],
            store: Default::default(),
        },
    )
    .unwrap();
    (child, input, output)
}

#[test]
fn installed_binary_executes_parallel_tool_requests_over_ipc() {
    let (mut child, mut input, mut output) = worker("const xs = await Promise.all([tools.echo({n:1}), tools.echo({n:2})]); store('xs', xs); return xs");
    for id in 1..=2 {
        let event: WorkerEvent = protocol::read_frame(&mut output).unwrap();
        assert!(
            matches!(event, WorkerEvent::Call { id: found, ref name, .. } if found == id && name == "echo")
        );
    }
    for id in (1..=2).rev() {
        protocol::write_frame(
            &mut input,
            &ToolReply {
                id,
                value: json!(id),
                error: None,
            },
        )
        .unwrap();
    }
    let event: WorkerEvent = protocol::read_frame(&mut output).unwrap();
    assert!(matches!(event, WorkerEvent::Output { ref value, .. } if value == "[1,2]"));
    let event: WorkerEvent = protocol::read_frame(&mut output).unwrap();
    let WorkerEvent::Done { error, writes } = event else {
        panic!("missing completion");
    };
    assert!(error.is_none());
    assert_eq!(writes["xs"], json!({"value":[1,2]}));
    assert!(child.wait().unwrap().success());
}

#[test]
fn native_worker_interrupts_loops_and_a_new_worker_remains_usable() {
    let (mut child, _input, mut output) = worker("text('partial'); while(true) {}");
    let event: WorkerEvent = protocol::read_frame(&mut output).unwrap();
    assert!(matches!(event, WorkerEvent::Output { ref value, .. } if value == "partial"));
    let event: WorkerEvent = protocol::read_frame(&mut output).unwrap();
    assert!(
        matches!(event, WorkerEvent::Done { error: Some(ref error), .. } if error.contains("timed out"))
    );
    assert!(child.wait().unwrap().success());
    let (mut child, _input, mut output) = worker("return 42");
    let event: WorkerEvent = protocol::read_frame(&mut output).unwrap();
    assert!(matches!(event, WorkerEvent::Output { ref value, .. } if value == "42"));
    let _: WorkerEvent = protocol::read_frame(&mut output).unwrap();
    assert!(child.wait().unwrap().success());
}
