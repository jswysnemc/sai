use crate::{catalog::CatalogEntry, protocol::*, worker::execute_stream};
use serde_json::json;
use std::io::Cursor;

/// 【CodeMode】【测试执行】用 code 与 replies 驱动沙箱，返回完整事件序列。
fn run(code: &str, replies: &[ToolReply], timeout_ms: u64) -> Vec<WorkerEvent> {
    let mut input = Vec::new();
    for reply in replies {
        write_frame(&mut input, reply).unwrap();
    }
    let request = ExecutionRequest {
        code: code.into(),
        timeout_ms,
        catalog: vec![CatalogEntry::new(
            "echo".into(),
            "echo text".into(),
            json!({"type":"object"}),
        )],
        store: Default::default(),
    };
    let bytes = execute_stream(request, &mut Cursor::new(input)).unwrap();
    let mut reader = Cursor::new(bytes);
    let mut result = Vec::new();
    while reader.position() < reader.get_ref().len() as u64 {
        result.push(read_frame(&mut reader).unwrap());
    }
    result
}

/// 【CodeMode】【结果断言】从 events 获取完成状态，返回错误与状态增量。
fn completion(events: &[WorkerEvent]) -> (&Option<String>, &crate::store::Store) {
    match events.last().unwrap() {
        WorkerEvent::Done { error, writes } => (error, writes),
        other => panic!("missing completion: {other:?}"),
    }
}

#[test]
fn async_body_and_output_work() {
    let events = run(
        "text('hello'); console.log('info', 2); return {n: await Promise.resolve(3)}",
        &[],
        1000,
    );
    assert!(completion(&events).0.is_none(), "{events:?}");
    assert!(matches!(&events[0], WorkerEvent::Output { value, .. } if value == "hello"));
    assert!(matches!(&events[2], WorkerEvent::Output { value, .. } if value == "{\"n\":3}"));
}

#[test]
fn tools_and_promise_all_resolve_in_requested_order() {
    let replies = vec![
        ToolReply {
            id: 2,
            value: json!("b"),
            error: None,
        },
        ToolReply {
            id: 1,
            value: json!("a"),
            error: None,
        },
    ];
    let events = run(
        "return await Promise.all([tools.echo({n:1}), tools.echo({n:2})])",
        &replies,
        1000,
    );
    assert!(completion(&events).0.is_none(), "{events:?}");
    assert!(matches!(&events[2], WorkerEvent::Output { value, .. } if value == "[\"a\",\"b\"]"));
}

#[test]
fn failure_keeps_partial_output_and_discards_store_writes() {
    let events = run(
        "store('x', null); text('partial'); throw new Error('boom')",
        &[],
        1000,
    );
    assert!(completion(&events).0.as_ref().unwrap().contains("boom"));
    assert!(completion(&events).1.is_empty());
    assert!(matches!(&events[0], WorkerEvent::Output { value, .. } if value == "partial"));
}

#[test]
fn successful_store_is_json_and_null_is_not_deletion() {
    let events = run("store('__proto__', {x:1}); store('x', null); store('y', undefined); return load('__proto__')", &[], 1000);
    assert!(completion(&events).0.is_none(), "{events:?}");
    let writes = completion(&events).1;
    assert_eq!(writes["x"], json!({"value":null}));
    assert_eq!(writes["y"], json!(null));
    let state = crate::store::apply(&Default::default(), writes).unwrap();
    assert_eq!(state["__proto__"], json!({"x":1}));
    assert!(state.contains_key("x"));
    assert!(!state.contains_key("y"));
}

#[test]
fn tool_errors_reject_and_can_be_caught() {
    let events = run(
        "try {await tools.echo({})} catch(e) {return e.message}",
        &[ToolReply {
            id: 1,
            value: json!(null),
            error: Some("denied".into()),
        }],
        1000,
    );
    assert!(completion(&events).0.is_none(), "{events:?}");
    assert!(matches!(&events[1], WorkerEvent::Output { value, .. } if value == "denied"));
}

#[test]
fn sandbox_has_no_os_apis_and_prototypes_are_frozen() {
    let events = run("return [typeof process, typeof require, typeof fetch, typeof setTimeout, Object.isFrozen(Object.prototype), Object.isFrozen(tools)]", &[], 1000);
    assert!(completion(&events).0.is_none(), "{events:?}");
    assert!(
        matches!(&events[0], WorkerEvent::Output { value, .. } if value == "[\"undefined\",\"undefined\",\"undefined\",\"undefined\",true,true]")
    );
    let events = run("Object.prototype.polluted = true", &[], 1000);
    assert!(completion(&events).0.is_some());
}

#[test]
fn loops_are_interrupted_and_stalled_promises_fail() {
    // 1. 【CodeMode】【执行预算】给原型冻结留出启动时间，部分输出断言只覆盖已开始的用户脚本
    let events = run("text('before'); while(true) {}", &[], 1000);
    assert!(
        completion(&events)
            .0
            .as_ref()
            .unwrap()
            .contains("timed out"),
        "{events:?}"
    );
    assert!(matches!(&events[0], WorkerEvent::Output { value, .. } if value == "before"));
    let events = run("await new Promise(() => {})", &[], 1000);
    assert!(completion(&events).0.as_ref().unwrap().contains("stalled"));
}

/// 【CodeMode】【初始化超时回归】无参数，返回无；已耗尽预算进入真实初始化路径时输出超时错误。
#[test]
fn initialization_timeout_is_reported_without_requiring_script_output() {
    let request = ExecutionRequest {
        code: "text('must-not-run'); return 1".into(),
        timeout_ms: 1000,
        catalog: Vec::new(),
        store: Default::default(),
    };
    let (sender, receiver) = std::sync::mpsc::sync_channel(16);
    crate::worker::execute_vm(
        request,
        &sender,
        &mut || panic!("no tools during initialization"),
        std::time::Instant::now() - std::time::Duration::from_millis(1),
    )
    .unwrap();
    drop(sender);
    let events = receiver.into_iter().collect::<Vec<_>>();
    assert_eq!(events.len(), 1, "{events:?}");
    assert!(
        completion(&events)
            .0
            .as_ref()
            .unwrap()
            .contains("timed out"),
        "{events:?}"
    );
    assert!(completion(&events).1.is_empty());
}

#[test]
fn discovery_and_exit_work() {
    let events = run("text((await searchTools('echo'))[0].name); text(await describeTool('echo')); store('x', 2); exit(); text('unreachable')", &[], 1000);
    assert!(completion(&events).0.is_none(), "{events:?}");
    assert_eq!(events.len(), 3);
    let WorkerEvent::Output { value, .. } = &events[1] else {
        panic!("missing tool description");
    };
    let description: serde_json::Value = serde_json::from_str(value).unwrap();
    assert_eq!(description["parameters"], json!({"type":"object"}));
    assert!(description.get("declaration").is_none());
    assert_eq!(completion(&events).1["x"], json!({"value":2}));
}

#[test]
fn invalid_images_and_oversized_store_fail() {
    for code in [
        "image('https://example.org/a.png')",
        "store('x', 'x'.repeat(262145))",
    ] {
        assert!(completion(&run(code, &[], 1000)).0.is_some());
    }
}
