use super::{prepare_at, Entry, Target};
use std::{path::PathBuf, sync::atomic::AtomicBool};

/// 【回收站测试】【独立材料】所有源文件和回收站均位于本测试创建的临时目录
pub(super) struct Fixture {
    pub root: tempfile::TempDir,
    pub path: PathBuf,
    pub data: PathBuf,
}

impl Fixture {
    /// 【回收站测试】【材料创建】创建任意名称与原始字节的普通文件
    /// @param name 文件名；bytes 为原始正文
    /// @returns 不修改真实用户回收站的测试材料
    pub(super) fn new(name: &str, bytes: &[u8]) -> Self {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("input").join(name);
        let data = root.path().join("data");
        std::fs::create_dir(path.parent().unwrap()).unwrap();
        std::fs::write(&path, bytes).unwrap();
        Self { root, path, data }
    }

    /// 【回收站测试】【准备操作】为本测试普通文件准备独立回收站条目
    /// @returns 目标句柄与尚未移动文件的条目
    pub(super) fn prepare(&self) -> (Target, Entry) {
        let target = Target::open(self.path.canonicalize().unwrap())
            .unwrap()
            .unwrap();
        let entry = prepare_at(&target, &self.data, &AtomicBool::new(false)).unwrap();
        (target, entry)
    }

    /// 【回收站测试】【条目位置】返回测试回收站中的文件和信息路径
    /// @param entry 测试条目
    /// @returns 文件路径及信息文件路径
    pub(super) fn paths(&self, entry: &Entry) -> (PathBuf, PathBuf) {
        let root = self.data.join("Trash");
        (
            root.join("files").join(&entry.reservation.name),
            root.join("info")
                .join(format!("{}.trashinfo", entry.reservation.name)),
        )
    }
}
