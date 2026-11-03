//! 多文件精确补丁基础工具，与现有编辑工具、权限链路和差异视图共用接口。

#[cfg(test)]
mod integration_tests;
mod model;
mod parser;
#[cfg(test)]
mod parser_tests;
mod prepare;
mod result;
mod target;
mod text;
mod transaction;
#[cfg(test)]
mod transaction_tests;

use crate::tools::{ToolRegistry, ToolSpec, file_edit::locks::FileEditLocks};
use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

struct Cancellation(Arc<AtomicBool>);

impl Drop for Cancellation {
    /// 【文件补丁】【取消通知】调用结束或被取消时阻止尚未开始提交的任务。
    /// 参数: 无；返回: 无。
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

/// 【文件补丁】【工具注册】注册常驻写入工具，使用现有权限审批和审计流程。
/// 参数: registry 为工具注册表；返回: 无。
pub(crate) fn register(registry: &mut ToolRegistry) {
    registry.register(ToolSpec::new("apply_patch", DESCRIPTION, json!({
        "type":"object", "properties":{"patch":{"type":"string","minLength":1,"maxLength":model::MAX_PATCH_BYTES,
        "description":"Complete patch: *** Begin Patch, one or more *** Add File: path (+lines), *** Update File: path (optional *** Move to: path, then @@ hunks with space/context, -removed, +added lines), or *** Delete File: path, then *** End Patch. @@ anchor selects a unique preceding line. *** End of File anchors the preceding hunk to EOF. Optional \\ No newline at end of file follows its content line. Do not wrap in code fences."}},
        "required":["patch"],"additionalProperties":false
    }), execute).writes());
}

const DESCRIPTION: &str = "Apply one exact patch to multiple text files. Prefer this for multiple related edits instead of repeated str_replace calls. Supports add, update, delete, and move without overwriting move destinations. Read existing files first; context must be exact and unique. Preserves existing LF/CRLF, BOM and final-newline style. Rejects duplicate targets, parent traversal, symlinks, hard links, binary files, mixed line endings during content edits, patches over 1 MiB, more than 64 file operations, files over 8 MiB or 200000 lines, and snapshots over 32 MiB. Excessive hunk matching work is rejected; combine nearby edits into fewer hunks. Shared edit locks coordinate Sai writers; snapshots detect outside edits before commit. All changes are validated before writing. Multi-file commits are not crash-atomic; runtime failures attempt rollback and report incomplete recovery. External programs that ignore the locks can still race; never retry a partial failure blindly.";

/// 【文件补丁】【后台执行】捕获工作目录，把锁等待和文件操作移出异步运行线程。
/// 参数: args 为经过 Schema 校验的参数；返回: 实际修改摘要。
async fn execute(args: Value) -> Result<String> {
    let patch = args
        .get("patch")
        .and_then(Value::as_str)
        .context("patch is required")?
        .to_string();
    let directory = crate::runtime_cwd::current_dir()?;
    let cancelled = Cancellation(Arc::new(AtomicBool::new(false)));
    let flag = cancelled.0.clone();
    tokio::task::spawn_blocking(move || {
        crate::runtime_cwd::with_directory(directory, || {
            let operations = parser::parse(&patch)?;
            let _locks = FileEditLocks::acquire(&prepare::paths(&operations)?)?;
            let mut plan = prepare::prepare(&operations)?;
            // 1. 【文件补丁】【结果预计算】避免写盘完成后才因差异计算或序列化失败丢失摘要
            let output = result::output(&plan)?;
            transaction::commit(&mut plan, &flag)?;
            Ok::<_, anyhow::Error>(output)
        })
    })
    .await
    .context("patch worker failed")?
    .map_err(|error| anyhow::anyhow!("{error:#}"))
}

/// 【文件补丁】【只读预览】通过同一解析器和上下文规则生成审批预览。
/// 参数: patch 为完整补丁；返回: 统一差异模型，不创建目录或文件。
pub(crate) fn preview(patch: &str) -> Result<crate::tools::file_change_model::AppliedPatch> {
    let operations = parser::parse(patch)?;
    Ok(result::preview(&prepare::prepare(&operations)?))
}

/// 【文件补丁】【读取门禁】提取需要校验读取快照的已有源文件，新增目标不需要先读取。
/// 参数: patch 为完整补丁；返回: 更新和删除涉及的源路径。
pub(crate) fn existing_targets(patch: &str) -> Result<Vec<PathBuf>> {
    parser::parse(patch)?
        .into_iter()
        .filter(|operation| !matches!(operation, model::Operation::Add { .. }))
        .map(|operation| target::resolve(operation.path()))
        .collect()
}
