//! 应用定向和快照生命周期：每次输入都必须以当前观察为依据。

use super::driver::NativeDriver;
use super::{accessibility, response, schema};
use crate::tools::ToolOutput;
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

/// 进程内的桌面互斥与变更代号，防止不同会话并发使用旧截图。
#[derive(Default)]
pub(super) struct DesktopCoordinator {
    gate: Mutex<()>,
    epoch: AtomicU64,
}

/// 一个确定的原生窗口，不允许退回当前前台窗口。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct WindowTarget {
    pub(super) pid: u32,
    pub(super) window_id: u64,
}

/// 仅当前会话可消费的观察证明，原生 token 不直接暴露给模型。
struct Snapshot {
    id: String,
    selector: String,
    target: WindowTarget,
    epoch: u64,
    elements: HashMap<String, String>,
    frame: Option<super::frame::ScreenshotFrame>,
    element_data: HashMap<String, Value>,
    capture_id: Option<String>,
}

/// 一份工具注册表持有一个原生会话，克隆注册表不会丢失快照边界。
pub(super) struct ComputerSession {
    driver: Arc<dyn NativeDriver>,
    coordinator: Arc<DesktopCoordinator>,
    snapshot: Option<Snapshot>,
    generation: u64,
}

impl ComputerSession {
    /// 【桌面操作】【会话构造】组合驱动与协调器，初始没有可使用的元素句柄。
    /// @param driver 为原生实现；coordinator 为同进程桌面互斥状态
    /// @returns 新会话
    pub(super) fn new(driver: Arc<dyn NativeDriver>, coordinator: Arc<DesktopCoordinator>) -> Self {
        Self {
            driver,
            coordinator,
            snapshot: None,
            generation: 0,
        }
    }

    /// 【桌面操作】【动作分派】先校验参数，再独占桌面请求；不排队重放旧快照动作。
    /// @param args 为统一工具参数
    /// @returns 工具文本与可选截图附件
    pub(super) async fn execute(&mut self, args: Value) -> Result<ToolOutput> {
        schema::validate(&args)?;
        let coordinator = self.coordinator.clone();
        let _guard = coordinator.gate.try_lock().map_err(|_| anyhow::anyhow!(
            "computer_use_busy: another session is using the desktop; observe again after it finishes"
        ))?;
        match args["action"]
            .as_str()
            .context("missing Computer Use action")?
        {
            "reset" => {
                self.snapshot = None;
                self.driver.close().await;
                Ok(ToolOutput::text("Computer Use session closed. Select an app/window and get_app_state before continuing."))
            }
            "list_apps" => {
                self.snapshot = None;
                self.read("list_apps", json!({})).await
            }
            "list_windows" => {
                self.snapshot = None;
                let pid = self.resolve_app(args["app"].as_str().unwrap()).await?;
                self.read("list_windows", json!({"pid":pid})).await
            }
            "get_app_state" => self.observe(&args).await,
            "get_value" | "get_text" | "locate_text" => {
                let snapshot = self.current_snapshot(&args)?;
                let data = accessibility::query(&args, &snapshot.element_data)?;
                Ok(ToolOutput::text(format!("Computer Use accessibility result\nsnapshot_id: {}\n<app_state>\n{}\n</app_state>", snapshot.id, serde_json::to_string(&data)?)))
            }
            "click" | "scroll" | "drag" | "press_key" | "type_text" | "set_value" | "invoke"
            | "show_menu" => self.input(&args).await,
            _ => bail!("Computer Use action is not executable in a native session"),
        }
    }

    /// 【桌面操作】【只读请求】读取应用或窗口列表，明确传播原生拒绝原因。
    /// @param name 为查询工具名；args 为原生查询参数
    /// @returns 列表文本
    async fn read(&self, name: &str, args: Value) -> Result<ToolOutput> {
        let result = self.driver.call(name, args).await?;
        response::check_error(&result)?;
        Ok(ToolOutput::text(response::text(&result)))
    }

    /// 【桌面操作】【目标解析】接受明确进程号或唯一运行应用名，不启动应用也不猜测前台目标。
    /// @param selector 为用户指定的应用标识
    /// @returns 正数进程号
    async fn resolve_app(&self, selector: &str) -> Result<u32> {
        let selector = selector.trim();
        if selector.bytes().all(|byte| byte.is_ascii_digit()) {
            let pid: u32 = selector.parse().context("app process id is out of range")?;
            if pid == 0 {
                bail!("app process id must be positive");
            }
            return Ok(pid);
        }
        let result = self.driver.call("list_apps", json!({})).await?;
        let data = response::structured(&result)?;
        let apps = data
            .get("apps")
            .or_else(|| data.get("applications"))
            .unwrap_or(&data)
            .as_array()
            .context(
                "list_apps did not return a structured app list; use an explicit process id",
            )?;
        let mut matches = apps
            .iter()
            .filter(|app| app.get("running").and_then(Value::as_bool) != Some(false))
            .filter(|app| {
                ["name", "app_name", "bundle_id", "bundleId"]
                    .iter()
                    .any(|key| {
                        app.get(key)
                            .and_then(Value::as_str)
                            .is_some_and(|value| value == selector)
                    })
            })
            .filter_map(|app| app.get("pid").and_then(Value::as_u64))
            .filter_map(|pid| u32::try_from(pid).ok())
            .filter(|pid| *pid > 0)
            .collect::<Vec<_>>();
        matches.sort_unstable();
        matches.dedup();
        match matches.as_slice() {
            [pid] => Ok(*pid),
            [] => bail!("No running app exactly matches {selector:?}; use list_apps and an explicit process id"),
            _ => bail!("Multiple running apps match {selector:?}; select an explicit process id from list_apps"),
        }
    }

    /// 【桌面操作】【窗口观察】丢弃旧观察，读取目标窗口并签发新的会话快照。
    /// @param args 为应用、窗口及观察选项
    /// @returns 原生状态与按观察选项处理的窗口截图
    async fn observe(&mut self, args: &Value) -> Result<ToolOutput> {
        self.snapshot = None;
        let selector = args["app"].as_str().unwrap().trim().to_owned();
        let target = WindowTarget {
            pid: self.resolve_app(&selector).await?,
            window_id: schema::integer(args, "window_id")?,
        };
        let mut payload = json!({
            "pid":target.pid, "window_id":target.window_id,
            "include_screenshot":args["include_screenshot"].as_bool().unwrap_or(true),
            "max_elements":args.get("max_elements").map(|_| schema::integer(args, "max_elements")).transpose()?.unwrap_or(500),
            "max_image_dimension":args.get("max_image_dimension").map(|_| schema::integer(args, "max_image_dimension")).transpose()?.unwrap_or(1568)
        });
        if let Some(query) = args.get("query") {
            payload["query"] = query.clone();
        }
        let result = self.driver.call("get_window_state", payload).await?;
        self.generation = self
            .generation
            .checked_add(1)
            .context("Computer Use generation exhausted")?;
        let id = uuid::Uuid::new_v4().to_string();
        let observed = response::observation(&result, target, self.generation, &id, args)?;
        self.snapshot = Some(Snapshot {
            id,
            selector,
            target,
            epoch: self.coordinator.epoch.load(Ordering::SeqCst),
            elements: observed.elements,
            frame: observed.frame,
            element_data: observed.element_data,
            capture_id: observed.capture_id,
        });
        Ok(observed.output)
    }

    /// 【桌面操作】【快照核对】验证查询或输入的目标与变更代号，拒绝跨会话和跨窗口引用。
    /// 参数: `args` 为带目标和快照的参数；返回: 当前快照，过期或目标不符时返回错误。
    fn current_snapshot(&self, args: &Value) -> Result<&Snapshot> {
        let snapshot = self
            .snapshot
            .as_ref()
            .context("No current Computer Use snapshot; call get_app_state first")?;
        if args["snapshot_id"].as_str() != Some(&snapshot.id)
            || snapshot.epoch != self.coordinator.epoch.load(Ordering::SeqCst)
        {
            bail!("stale_snapshot: call get_app_state again before input");
        }
        let app = args["app"].as_str().unwrap().trim();
        if (app != snapshot.selector && app != snapshot.target.pid.to_string())
            || schema::integer(args, "window_id")? != snapshot.target.window_id
        {
            bail!("snapshot_target_mismatch: observe the requested app/window first");
        }
        Ok(snapshot)
    }

    /// 【桌面操作】【快照输入】验证目标与引用，发送前消费快照，取消或失败后也不能重复使用。
    /// @param args 为完整输入参数
    /// @returns 原生结果和重新观察提示
    async fn input(&mut self, args: &Value) -> Result<ToolOutput> {
        let snapshot = self.current_snapshot(args)?;
        let action = args["action"].as_str().unwrap();
        let payload = input_payload(args, snapshot)?;
        // 1. 【桌面操作】【先行失效】无论驱动成功、拒绝还是取消，本次输入都不能自动重放
        self.snapshot = None;
        self.coordinator.epoch.fetch_add(1, Ordering::SeqCst);
        let native_action = if matches!(action, "invoke" | "show_menu") {
            "click"
        } else {
            action
        };
        let result = self.driver.call(native_action, payload).await?;
        response::check_error(&result)?;
        Ok(ToolOutput::text(format!(
            "{}\nCall get_app_state for the updated state before choosing another action. The previous snapshot has been consumed.",
            response::text(&result)
        )))
    }
}

/// 【桌面操作】【输入报文】把公开快照句柄转换为原生 token，校验像素后固定后台投递。
/// @param args 为公开参数；snapshot 为当前观察
/// @returns 不含宿主配置或任意原生方法名的输入报文
fn input_payload(args: &Value, snapshot: &Snapshot) -> Result<Value> {
    let action = args["action"].as_str().unwrap();
    let mut payload = json!({
        "pid":snapshot.target.pid, "window_id":snapshot.target.window_id,
        "delivery_mode":"background"
    });
    if matches!(action, "invoke" | "show_menu") {
        payload["action"] = json!(if action == "invoke" {
            "press"
        } else {
            "show_menu"
        });
    }
    if let Some(handle) = args.get("element_index").and_then(Value::as_str) {
        let token = snapshot
            .elements
            .get(handle)
            .context("stale_element: copy an element_index from the current snapshot")?;
        payload["element_token"] = json!(token);
    }
    for (x_key, y_key) in [("x", "y"), ("from_x", "from_y"), ("to_x", "to_y")] {
        if let Some(x) = args.get(x_key).and_then(Value::as_f64) {
            let y = args[y_key].as_f64().context("missing paired coordinate")?;
            let frame = snapshot.frame.as_ref().context(
                "screenshot_required: pixel input requires a screenshot of this exact window",
            )?;
            let (x, y) = frame.project(x, y)?;
            payload[x_key] = json!(x);
            payload[y_key] = json!(y);
            payload["coordinate_frame"] = json!("window");
        }
    }
    if action == "click" && args.get("x").is_some() {
        if let Some(id) = &snapshot.capture_id {
            payload["capture_id"] = json!(id);
        }
    }
    for key in [
        "button",
        "direction",
        "amount",
        "by",
        "duration_ms",
        "text",
        "value",
        "key",
    ] {
        if let Some(value) = args.get(key) {
            payload[key] = if matches!(key, "amount" | "duration_ms") {
                json!(schema::integer(args, key)?)
            } else {
                value.clone()
            };
        }
    }
    if args.get("click_count").is_some() {
        payload["count"] = json!(schema::integer(args, "click_count")?);
    }
    if let Some(modifiers) = args.get("modifiers") {
        payload[if action == "click" {
            "modifier"
        } else {
            "modifiers"
        }] = modifiers.clone();
    }
    Ok(payload)
}
