//! 文件上传：面板把用户选的文件上传到 Sai 临时目录，再交给页面的文件输入框。

use super::events::BrowserEvent;
use super::session::BrowserSession;
use anyhow::{bail, Context, Result};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Mutex;

/// 单个上传文件的大小上限。
pub(crate) const MAX_UPLOAD_BYTES: usize = 100 << 20;

/// 本进程上传文件的临时目录；浏览器关闭时删除，下次上传时重建。
static UPLOAD_DIR: Mutex<Option<tempfile::TempDir>> = Mutex::new(None);

/// 【内置浏览器】【上传目录】返回本进程专用的上传临时目录，不存在时创建。
/// @returns 目录路径；创建失败时为空
fn upload_dir() -> Option<PathBuf> {
    let mut slot = UPLOAD_DIR.lock().unwrap();
    if slot.is_none() {
        *slot = tempfile::Builder::new()
            .prefix("sai-browser-upload-")
            .tempdir()
            .ok();
    }
    slot.as_ref().map(|dir| dir.path().to_path_buf())
}

/// 【内置浏览器】【上传清理】删除上传临时目录；浏览器已关闭，输入框中的文件不再需要。
/// @returns 无
pub(crate) fn remove_uploads() {
    UPLOAD_DIR.lock().unwrap().take();
}

/// 【内置浏览器】【上传保存】把一个上传文件写入临时目录。
///
/// 每个文件放在独立的随机子目录中，保留原文件名，页面读到的文件名与用户选择一致。
///
/// @param file_name 为原文件名；bytes 为文件内容
/// @returns 供回复文件选择时引用的上传 ID
pub(crate) fn store_upload(file_name: &str, bytes: &[u8]) -> Result<String> {
    if bytes.len() > MAX_UPLOAD_BYTES {
        bail!("file is larger than {} MB", MAX_UPLOAD_BYTES >> 20);
    }
    let root = upload_dir().context("upload directory is unavailable")?;
    let id = uuid::Uuid::new_v4().simple().to_string();
    let dir = root.join(&id);
    std::fs::create_dir_all(&dir).context("create upload directory")?;
    let name = super::downloads::sanitize_file_name(file_name);
    std::fs::write(dir.join(&name), bytes).context("write uploaded file")?;
    Ok(id)
}

/// 【内置浏览器】【上传解析】把上传 ID 解析为本地文件；只接受本进程生成的 ID。
/// @param id 为上传 ID
/// @returns 本地文件路径
fn resolve_upload(id: &str) -> Result<PathBuf> {
    if id.len() != 32 || !id.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!("invalid upload id");
    }
    let dir = upload_dir()
        .context("upload directory is unavailable")?
        .join(id);
    let file = std::fs::read_dir(&dir)
        .with_context(|| format!("upload {id} not found"))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .find(|path| path.is_file())
        .with_context(|| format!("upload {id} is empty"))?;
    Ok(file)
}

/// 【内置浏览器】【上传解析】测试入口。
/// @param id 为上传 ID
/// @returns 本地文件路径
#[cfg(test)]
pub(super) fn resolve_upload_for_test(id: &str) -> Result<PathBuf> {
    resolve_upload(id)
}

impl BrowserSession {
    /// 【内置浏览器】【文件选择回复】把上传的文件交给页面的文件输入框；空列表表示取消。
    /// @param ids 为上传 ID 列表
    /// @returns 操作结果；没有等待中的文件选择时报错
    pub(crate) async fn answer_file_chooser(&self, ids: &[String]) -> Result<()> {
        let Some((session, node)) = self.inner.lock().unwrap().file_chooser.take() else {
            bail!("no file chooser is open");
        };
        let _ = self.events.send(BrowserEvent::FileChooser(None));
        if ids.is_empty() {
            return Ok(());
        }
        let files = ids
            .iter()
            .map(|id| resolve_upload(id).map(|path| path.display().to_string()))
            .collect::<Result<Vec<_>>>()?;
        self.client
            .send(
                Some(&session),
                "DOM.setFileInputFiles",
                json!({ "files": files, "backendNodeId": node }),
            )
            .await?;
        Ok(())
    }

    /// 【内置浏览器】【文件选择查询】是否有等待面板选择文件的输入框。
    /// @returns 有等待中的文件选择时为 true
    pub(crate) fn file_chooser_pending(&self) -> bool {
        self.inner.lock().unwrap().file_chooser.is_some()
    }
}
