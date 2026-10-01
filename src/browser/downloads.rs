//! 页面下载：文件落到 Sai 管理的目录，面板通过下载接口把文件交给用户本机浏览器。

use super::events::{BrowserEvent, DownloadInfo};
use super::session::BrowserSession;
use serde_json::{json, Value};
use std::path::PathBuf;

/// 面板列表中保留的最近下载数量。
const MAX_DOWNLOADS: usize = 20;

impl BrowserSession {
    /// 【内置浏览器】【下载设置】把下载写入 Sai 数据目录，并开启下载进度事件。
    /// @returns 无；目录不可用时保留浏览器默认行为
    pub(super) async fn configure_downloads(&self) {
        let Some(dir) = download_dir() else {
            return;
        };
        if std::fs::create_dir_all(&dir).is_err() {
            return;
        }
        let _ = self
            .client
            .send(
                None,
                "Browser.setDownloadBehavior",
                json!({
                    // 按 GUID 命名，避免同名文件互相覆盖；原文件名在事件中保留
                    "behavior": "allowAndName",
                    "downloadPath": dir.display().to_string(),
                    "eventsEnabled": true,
                }),
            )
            .await;
    }

    /// 【内置浏览器】【下载开始】记录新下载并通知面板。
    /// @param params 为 Browser.downloadWillBegin 参数
    /// @returns 无
    pub(super) fn download_began(&self, params: &Value) {
        let guid = text(params, "guid");
        if guid.is_empty() {
            return;
        }
        let info = DownloadInfo {
            guid: guid.clone(),
            file_name: sanitize_file_name(&text(params, "suggestedFilename")),
            url: text(params, "url"),
            state: "inProgress".to_string(),
            received: 0,
            total: 0,
        };
        let mut inner = self.inner.lock().unwrap();
        inner.downloads.retain(|item| item.guid != guid);
        inner.downloads.push(info.clone());
        let overflow = inner.downloads.len().saturating_sub(MAX_DOWNLOADS);
        inner.downloads.drain(..overflow);
        drop(inner);
        let _ = self.events.send(BrowserEvent::Download(info));
    }

    /// 【内置浏览器】【下载进度】更新下载进度；完成、取消时通知面板。
    /// @param params 为 Browser.downloadProgress 参数
    /// @returns 无
    pub(super) fn download_progressed(&self, params: &Value) {
        let guid = text(params, "guid");
        let state = text(params, "state");
        let mut inner = self.inner.lock().unwrap();
        let Some(item) = inner.downloads.iter_mut().find(|item| item.guid == guid) else {
            return;
        };
        item.received = params
            .get("receivedBytes")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        item.total = params
            .get("totalBytes")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let finished = state != item.state && state != "inProgress";
        item.state = state;
        let info = item.clone();
        drop(inner);
        // 进度事件非常密集，只在状态变化时推送
        if finished {
            let _ = self.events.send(BrowserEvent::Download(info));
        }
    }

    /// 【内置浏览器】【下载查询】返回已完成下载的本地文件与原文件名。
    /// @param guid 为下载 GUID
    /// @returns (本地路径, 原文件名)；未完成或不存在时为空
    pub(crate) fn completed_download(&self, guid: &str) -> Option<(PathBuf, String)> {
        let inner = self.inner.lock().unwrap();
        let item = inner
            .downloads
            .iter()
            .find(|item| item.guid == guid && item.state == "completed")?;
        let path = download_dir()?.join(&item.guid);
        Some((path, item.file_name.clone()))
    }
}

/// 【内置浏览器】【下载目录】返回下载目录：持久用户目录的同级 downloads。
/// @returns 下载目录；无法确定数据目录时为空
fn download_dir() -> Option<PathBuf> {
    Some(
        super::profile::persistent_root()?
            .parent()?
            .join("downloads"),
    )
}

/// 【内置浏览器】【文件名清理】去掉路径分隔符与控制字符，避免响应头注入。
/// @param name 为页面建议的文件名
/// @returns 安全的文件名；为空时返回 download
pub(crate) fn sanitize_file_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .filter(|ch| !ch.is_control() && !matches!(ch, '/' | '\\' | '"'))
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').to_string();
    if cleaned.is_empty() {
        "download".to_string()
    } else {
        cleaned.chars().take(200).collect()
    }
}

/// 【内置浏览器】【字段读取】读取字符串字段。
/// @param value 为 JSON 对象；key 为字段名
/// @returns 字段值，缺失时为空串
fn text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}
