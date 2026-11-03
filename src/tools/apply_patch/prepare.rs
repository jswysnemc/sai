//! 先验证整份补丁并保存快照，准备阶段不创建目录或写入文件。

use super::{
    model::{MAX_FILE_BYTES, MAX_TOTAL_BYTES, Operation},
    target::{Snapshot, Target, resolve},
    text,
};
use anyhow::{Context, Result, ensure};
use std::{collections::BTreeSet, fs::Permissions, path::PathBuf};

pub(super) struct FileState {
    pub target: Target,
    pub before: Option<Snapshot>,
    pub after: Option<String>,
    pub permissions: Option<Permissions>,
}

pub(super) struct Change {
    pub action: &'static str,
    pub path: PathBuf,
    pub source: Option<PathBuf>,
    pub old: String,
    pub new: String,
}

pub(super) struct Plan {
    pub files: Vec<FileState>,
    pub changes: Vec<Change>,
}

/// 【文件补丁】【目标集合】校验重复路径、移动覆盖和父子路径冲突，提供锁定范围。
/// 参数: operations 为解析结果；返回: 所有源路径与移动目标。
pub(super) fn paths(operations: &[Operation]) -> Result<Vec<PathBuf>> {
    let mut paths = BTreeSet::new();
    for operation in operations {
        let mut candidates = vec![resolve(operation.path())?];
        if let Operation::Update {
            destination: Some(destination),
            ..
        } = operation
        {
            candidates.push(resolve(destination)?);
        }
        for path in candidates {
            ensure!(
                paths.insert(path.clone()),
                "duplicate or conflicting patch path: {}",
                path.display()
            );
        }
    }
    let paths = paths.into_iter().collect::<Vec<_>>();
    #[cfg(windows)]
    {
        let normalized = paths
            .iter()
            .map(|path| path.to_string_lossy().to_lowercase())
            .collect::<BTreeSet<_>>();
        ensure!(
            normalized.len() == paths.len(),
            "patch contains case-insensitive path aliases"
        );
    }
    for (index, path) in paths.iter().enumerate() {
        ensure!(
            !paths[index + 1..]
                .iter()
                .any(|other| other.starts_with(path)),
            "patch contains conflicting parent and child paths: {}",
            path.display()
        );
    }
    Ok(paths)
}

/// 【文件补丁】【执行计划】读取锁定文件并一次性验证全部修改，不接受二进制或空修改。
/// 参数: operations 为解析结果；返回: 包含恢复快照的完整执行计划。
pub(super) fn prepare(operations: &[Operation]) -> Result<Plan> {
    paths(operations)?;
    let mut plan = Plan {
        files: Vec::new(),
        changes: Vec::new(),
    };
    let mut total = 0usize;
    for operation in operations {
        let path = resolve(operation.path())?;
        let target = Target::new(path.clone())?;
        let before = target
            .snapshot()
            .with_context(|| format!("cannot prepare {}", path.display()))?;
        let old = before
            .as_ref()
            .map_or("", |snapshot| snapshot.content.as_str());
        let permissions = before
            .as_ref()
            .map(|snapshot| snapshot.metadata.permissions());
        let (action, destination, after) = match operation {
            Operation::Add { content, .. } => {
                ensure!(
                    before.is_none(),
                    "Add File refuses to overwrite existing file: {}",
                    path.display()
                );
                ("Added", None, Some(content.clone()))
            }
            Operation::Delete { .. } => {
                ensure!(
                    before.is_some(),
                    "Delete File target does not exist: {}",
                    path.display()
                );
                ("Deleted", None, None)
            }
            Operation::Update {
                destination, hunks, ..
            } => {
                ensure!(
                    before.is_some(),
                    "Update File target does not exist: {}",
                    path.display()
                );
                let updated = if hunks.is_empty() {
                    old.to_string()
                } else {
                    text::apply(old, hunks)
                        .with_context(|| format!("cannot apply hunks to {}", path.display()))?
                };
                ensure!(
                    destination.is_some() || old != updated,
                    "patch makes no changes to {}",
                    path.display()
                );
                (
                    if destination.is_some() {
                        "Renamed"
                    } else {
                        "Edited"
                    },
                    destination.as_deref().map(resolve).transpose()?,
                    Some(updated),
                )
            }
        };
        ensure!(
            after
                .as_ref()
                .is_none_or(|content| content.len() <= MAX_FILE_BYTES),
            "patched file exceeds the 8 MiB limit"
        );
        ensure!(
            after
                .as_ref()
                .is_none_or(|content| content.lines().count() <= 200_000),
            "patched file exceeds the 200000-line limit"
        );
        total += old.len() + after.as_ref().map_or(0, String::len);
        ensure!(
            total <= MAX_TOTAL_BYTES,
            "patch snapshots exceed the 32 MiB total limit"
        );
        plan.changes.push(Change {
            action,
            path: destination.as_ref().unwrap_or(&path).clone(),
            source: destination.as_ref().map(|_| path.clone()),
            old: old.to_string(),
            new: after.clone().unwrap_or_default(),
        });
        if let Some(destination) = destination {
            let new_target = Target::new(destination.clone())?;
            ensure!(
                new_target.snapshot()?.is_none(),
                "Move to refuses to overwrite existing path: {}",
                destination.display()
            );
            // 1. 【文件补丁】【移动顺序】先提交目的文件，再删除源文件；失败时按相反顺序恢复
            plan.files.push(FileState {
                target: new_target,
                before: None,
                after,
                permissions,
            });
            plan.files.push(FileState {
                target,
                before,
                after: None,
                permissions: None,
            });
        } else {
            plan.files.push(FileState {
                target,
                before,
                after,
                permissions,
            });
        }
    }
    Ok(plan)
}
