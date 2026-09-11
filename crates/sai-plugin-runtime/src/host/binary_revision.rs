/// 【插件二进制】【文件修订】区分目标不存在与完整普通文件的 SHA-256 摘要
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BinaryRevision {
    Missing,
    Sha256([u8; 32]),
}

/// 【插件二进制】【条件请求】由可信运行时构造，不允许 Lua 覆盖比较大小上限
#[derive(Clone, Debug)]
pub struct BinaryConditionalWrite {
    pub path: String,
    pub expected: BinaryRevision,
    pub max_bytes: usize,
}
