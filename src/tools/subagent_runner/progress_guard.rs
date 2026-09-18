use crate::agent::model_json::first_json_object;
use crate::agent::repeat_guard::RepeatVerdict;
use serde_json::Value;
use std::{collections::HashMap, time::Duration};
#[cfg(test)]
#[path = "progress_guard_tests.rs"]
mod tests;

const UNCHANGED_CALL_LIMIT: usize = 4;
const STALLED_ROUND_LIMIT: usize = 3;

/// 【子任务】【循环防护】同时跟踪调用结果和整轮进展，避免仅拦工具却继续空转。
#[derive(Default)]
pub(super) struct ProgressGuard {
    observations: HashMap<blake3::Hash, Observation>,
    stalled_rounds: usize,
    round_progress: bool,
}

#[derive(Default)]
struct Observation {
    result: Option<blake3::Hash>,
    unchanged: usize,
}

impl ProgressGuard {
    /// 【子任务】【循环防护】开始新的工具轮次；无参数、无返回值。
    pub(super) fn begin_round(&mut self) {
        self.round_progress = false;
    }

    /// 【子任务】【循环防护】执行前限制已确认没有变化的调用。
    /// 参数：`name` 为工具名，`arguments` 为真实参数；返回放行、提醒或停止。
    pub(super) fn observe(&self, name: &str, arguments: &str) -> RepeatVerdict {
        let count = self
            .observations
            .get(&call_key(name, arguments))
            .map_or(0, |observation| observation.unchanged);
        match count {
            UNCHANGED_CALL_LIMIT.. => RepeatVerdict::Stop { seen: count + 1 },
            2.. => RepeatVerdict::Warn { seen: count + 1 },
            _ => RepeatVerdict::Allow,
        }
    }

    /// 【子任务】【循环防护】记录实际结果，变化的内容重新获得推进机会。
    /// 参数：工具名、实际参数、成功状态、完整输出与调用耗时；返回无。
    pub(super) fn record(
        &mut self,
        name: &str,
        arguments: &str,
        ok: bool,
        output: &str,
        elapsed: Duration,
    ) {
        // 1. 有效阻塞等待属于正常进展，已经结束或瞬间返回的等待仍接受重复检查
        let args = first_json_object(arguments).ok();
        let value = serde_json::from_str::<Value>(output).ok();
        let waiting = name == "background_command"
            && ok
            && args.as_ref().is_some_and(|value| value["action"] == "wait")
            && value.as_ref().is_some_and(|value| {
                value["waited"] == true
                    && value["completed"] == false
                    && value["task"]["status"] == "running"
            })
            && elapsed >= Duration::from_secs(1);
        let key = call_key(name, arguments);
        if waiting {
            self.observations.remove(&key);
            self.round_progress = true;
            return;
        }
        // 2. 只比较实际结果，命令的审计编号变化不算获得新信息
        let digest = result_key(name, ok, output, value);
        let observation = self.observations.entry(key).or_default();
        if observation.result == Some(digest) {
            observation.unchanged += 1;
        } else {
            observation.result = Some(digest);
            observation.unchanged = 1;
            self.round_progress = true;
        }
    }

    /// 【子任务】【循环防护】整轮结束时决定是否收尾。
    /// 无参数；返回连续无进展轮次是否达到上限，交替重复的调用同样计入。
    pub(super) fn finish_round(&mut self) -> bool {
        self.stalled_rounds = if self.round_progress {
            0
        } else {
            self.stalled_rounds + 1
        };
        self.stalled_rounds >= STALLED_ROUND_LIMIT
    }
}

/// 【子任务】【循环防护】构造只含实际调用参数的稳定键。
/// 参数：工具名与参数文本；返回稳定哈希，避免保存大段原文。
fn call_key(name: &str, arguments: &str) -> blake3::Hash {
    let mut args = first_json_object(arguments).ok();
    if name == "run_command" {
        if let Some(Value::Object(map)) = args.as_mut() {
            map.remove("justification");
        }
    }
    let mut hasher = blake3::Hasher::new();
    hasher.update(name.as_bytes());
    hasher.update(&[0]);
    hasher.update(
        args.map(stable_json)
            .unwrap_or_else(|| arguments.trim().to_string())
            .as_bytes(),
    );
    hasher.finalize()
}

/// 【子任务】【循环防护】去除命令审计元数据后比较结果。
/// 参数：工具名、成功状态、原文与已解析 JSON；返回稳定结果哈希。
fn result_key(name: &str, ok: bool, output: &str, mut value: Option<Value>) -> blake3::Hash {
    if name == "run_command" {
        // 【子任务】【循环防护】新建后台任务的编号与时间戳不代表原命令获得了新结果
        if let Some(result) = value.as_mut().filter(|value| value["mode"] == "background") {
            *result = serde_json::json!({
                "mode": "background",
                "ok": result["ok"],
                "status": result["task"]["status"],
                "exit_code": result["task"]["exit_code"],
            });
        }
        if let Some(Value::Object(map)) =
            value.as_mut().filter(|value| value["mode"] == "foreground")
        {
            map.remove("task_id");
            map.remove("note");
        }
    }
    let mut hasher = blake3::Hasher::new();
    hasher.update(&[u8::from(ok)]);
    hasher.update(
        value
            .map(stable_json)
            .unwrap_or_else(|| output.to_string())
            .as_bytes(),
    );
    hasher.finalize()
}

/// 【子任务】【循环防护】递归排序 JSON 字段，数组顺序与字符串原样保留。
/// 参数：JSON 值；返回稳定序列化文本。
fn stable_json(mut value: Value) -> String {
    value.sort_all_objects();
    value.to_string()
}

/// 【子任务】【命令结果】区分成功调用工具与成功执行命令。
/// 参数：工具名、工具输出；返回命令是否成功，非命令工具沿用调用成功状态。
pub(super) fn tool_succeeded(name: &str, output: &str) -> bool {
    if !matches!(name, "run_command" | "background_command") {
        return true;
    }
    let Ok(value) = serde_json::from_str::<Value>(output) else {
        return true;
    };
    value["success"] != false
        && value["ok"] != false
        && !value["exit_code"].as_i64().is_some_and(|code| code != 0)
        && !value["task"]["exit_code"]
            .as_i64()
            .is_some_and(|code| code != 0)
}
