use super::assembler::EventAssembler;
use crate::agent::AgentEvent;
use crate::runner::RunnerEvent;

/// ACP 工具标题可能在结果阶段缺失，provider 调用标识仍要合并同一张卡。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn keeps_identified_tool_lifecycle_on_one_tool_id() {
    let mut assembler = EventAssembler::new("workspace", "session");
    assembler.begin_run("run", "", &[]);
    let started = assembler.map(RunnerEvent::Agent(AgentEvent::ToolCallIdentified {
        id: "provider-call-1".to_string(),
        name: "Read file".to_string(),
        arguments: r#"{"path":"README.md"}"#.to_string(),
    }));
    let result = assembler.map(RunnerEvent::Agent(AgentEvent::ToolResultIdentified {
        id: "provider-call-1".to_string(),
        name: "Read file".to_string(),
        ok: true,
        output: "content".to_string(),
    }));

    let started_id = started.last().unwrap().payload["tool_id"].as_str().unwrap();
    let result_id = result.last().unwrap().payload["tool_id"].as_str().unwrap();
    assert_eq!(started_id, result_id);
}
