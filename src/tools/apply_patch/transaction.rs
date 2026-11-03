//! 有界多文件提交：先暂存并复查全部快照，出错时恢复已提交文件并报告恢复失败。

use super::{
    prepare::Plan,
    target::{CreatedDirectory, Snapshot},
};
use anyhow::{Result, ensure};
use std::sync::atomic::{AtomicBool, Ordering};

struct Staging<'a> {
    plan: &'a mut Plan,
    files: Vec<Option<String>>,
    created: Vec<CreatedDirectory>,
}

impl Drop for Staging<'_> {
    /// 【文件补丁】【事务清理】无论成功、失败或提前返回，都移除本次暂存文件和空目录。
    /// 参数: 无；返回: 无。
    fn drop(&mut self) {
        for (file, staged) in self.plan.files.iter().zip(&self.files) {
            if let Some(staged) = staged {
                file.target.discard(staged);
            }
        }
        for directory in self.created.iter().rev() {
            directory.discard();
        }
    }
}

/// 【文件补丁】【事务执行】开始提交后不因异步调用取消而中断恢复流程。
/// 参数: plan 为已验证计划，cancelled 为调用取消标记；返回: 全部完成或明确的恢复结果。
pub(super) fn commit(plan: &mut Plan, cancelled: &AtomicBool) -> Result<()> {
    commit_with(plan, cancelled, |_| Ok(()))
}

/// 【文件补丁】【受控提交】支持在每次提交前注入故障，供回归测试覆盖真实恢复分支。
/// 参数: plan 为计划，cancelled 为取消标记，before_commit 为提交前检查；返回: 事务结果。
pub(super) fn commit_with(
    plan: &mut Plan,
    cancelled: &AtomicBool,
    mut before_commit: impl FnMut(usize) -> Result<()>,
) -> Result<()> {
    let mut staging = Staging {
        plan,
        files: Vec::new(),
        created: Vec::new(),
    };
    for file in &mut staging.plan.files {
        ensure!(
            !cancelled.load(Ordering::Acquire),
            "patch cancelled before commit; no file changes applied"
        );
        file.target.verify(file.before.as_ref())?;
        file.target.ensure_parent(&mut staging.created)?;
        let staged = file
            .after
            .as_ref()
            .map(|content| file.target.stage(content, file.permissions.clone()))
            .transpose()?;
        staging.files.push(staged);
    }
    // 1. 【文件补丁】【统一复查】暂存完成后复查全部目标，不能边验证边写入
    for file in &staging.plan.files {
        file.target.verify(file.before.as_ref())?;
    }
    ensure!(
        !cancelled.load(Ordering::Acquire),
        "patch cancelled before commit; no file changes applied"
    );
    let mut committed = Vec::<(usize, Option<Snapshot>)>::new();
    for index in 0..staging.plan.files.len() {
        let file = &staging.plan.files[index];
        let result = before_commit(index)
            .and_then(|_| file.target.verify(file.before.as_ref()))
            .and_then(|_| file.target.commit(staging.files[index].as_deref()));
        if let Err(error) = result {
            let mut failures = Vec::new();
            // 2. 【文件补丁】【失败恢复】不覆盖提交后又被外部编辑的文件，逐项报告保留位置
            for (done, written) in committed.into_iter().rev() {
                let file = &staging.plan.files[done];
                let restored = (|| -> Result<()> {
                    file.target.verify(written.as_ref())?;
                    if let Some(before) = &file.before {
                        let staged = file
                            .target
                            .stage(&before.content, Some(before.metadata.permissions()))?;
                        let result = file.target.commit(Some(&staged));
                        file.target.discard(&staged);
                        result
                    } else {
                        file.target.commit(None)
                    }
                })();
                if let Err(restore_error) = restored {
                    failures.push(format!("{}: {restore_error:#}", file.target.path.display()));
                }
            }
            if failures.is_empty() {
                anyhow::bail!("patch failed; all applied file changes were rolled back: {error:#}");
            }
            anyhow::bail!(
                "patch failed: {error:#}; ROLLBACK INCOMPLETE, inspect these paths before retrying: {}",
                failures.join("; ")
            );
        }
        staging.files[index] = None;
        // 3. 【文件补丁】【恢复基线】保存提交后的版本，恢复时识别外部并发修改
        let written = file.target.snapshot();
        match written {
            Ok(written)
                if written.as_ref().map(|snapshot| snapshot.content.as_str())
                    == file.after.as_deref() =>
            {
                committed.push((index, written))
            }
            Ok(_) => anyhow::bail!(
                "patch partially applied; external modification detected immediately after committing {}. Inspect all patch paths before retrying",
                file.target.path.display()
            ),
            Err(error) => anyhow::bail!(
                "patch partially applied; cannot verify committed path {}: {error:#}. Inspect all patch paths before retrying",
                file.target.path.display()
            ),
        }
    }
    Ok(())
}
