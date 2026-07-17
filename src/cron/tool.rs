use super::repository::CronRepository;
use crate::paths::SaiPaths;
use crate::tools::{ToolRegistry, ToolSpec};
use anyhow::{bail, Result};
use chrono::Utc;
use serde_json::{json, Value};

/// 注册仅供 Gateway Agent 使用的定时任务工具。
pub(crate) fn register(registry: &mut ToolRegistry, paths: SaiPaths, session_id: String) {
    registry.register(ToolSpec::new("cron", "Manage durable Gateway scheduled tasks. Managed gateway processes keep a dedicated scheduler running.", json!({
        "type":"object","properties":{
            "action":{"type":"string","enum":["list","create","remove"]},
            "id":{"type":"string"},"name":{"type":"string"},"prompt":{"type":"string"},
            "run_at":{"type":"integer","description":"Unix timestamp in seconds."},
            "delay_seconds":{"type":"integer"},"interval_seconds":{"type":"integer"}
        },"required":["action"],"additionalProperties":false
    }), move |args| { let paths=paths.clone(); let session_id=session_id.clone(); async move { execute(&paths,&session_id,args) } }).writes());
}

fn execute(paths: &SaiPaths, session_id: &str, args: Value) -> Result<String> {
    let repository = CronRepository::new(paths)?;
    match args
        .get("action")
        .and_then(Value::as_str)
        .unwrap_or_default()
    {
        "list" => Ok(json!({"ok":true,"jobs":repository.list()?}).to_string()),
        "create" => {
            let prompt = required(&args, "prompt")?;
            let name = args.get("name").and_then(Value::as_str).unwrap_or(prompt);
            let run_at = args
                .get("run_at")
                .and_then(Value::as_i64)
                .or_else(|| {
                    args.get("delay_seconds")
                        .and_then(Value::as_i64)
                        .map(|delay| Utc::now().timestamp().saturating_add(delay))
                })
                .unwrap_or_else(|| Utc::now().timestamp());
            let job = repository.create(
                name,
                prompt,
                session_id,
                run_at,
                args.get("interval_seconds").and_then(Value::as_i64),
            )?;
            Ok(json!({"ok":true,"job":job}).to_string())
        }
        "remove" => {
            let id = required(&args, "id")?;
            Ok(json!({"ok":repository.remove(id)?,"id":id}).to_string())
        }
        action => bail!("unsupported cron action: {action}"),
    }
}

fn required<'a>(args: &'a Value, name: &str) -> Result<&'a str> {
    args.get(name)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("{name} is required"))
}
