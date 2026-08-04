use serde::{Deserialize, Serialize};

/// 记忆条目的作用域。
///
/// 召回时按当前工作区过滤：在 A 项目里做的技术决策不应该污染 B 项目的上下文。
/// 这是「召回不准」的一个独立来源——旧实现全部记忆平铺在一个池子里。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MemoryScope {
    /// 跨项目通用：用户的沟通偏好、通用工具链选择
    Global,
    /// 仅在特定工作区内有效
    Project { path: String },
}

impl MemoryScope {
    /// 从存库字段还原作用域。
    ///
    /// 参数:
    /// - `path`: 项目路径；为空表示全局
    ///
    /// 返回:
    /// - 对应的作用域
    pub fn from_stored(path: Option<&str>) -> Self {
        match path.map(str::trim).filter(|value| !value.is_empty()) {
            Some(path) => Self::Project {
                path: path.to_string(),
            },
            None => Self::Global,
        }
    }

    /// 返回存库使用的路径字段。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 项目路径；全局作用域为 None
    pub fn stored_path(&self) -> Option<&str> {
        match self {
            Self::Global => None,
            Self::Project { path } => Some(path.as_str()),
        }
    }

    /// 判断该作用域是否对给定工作区可见。
    ///
    /// 全局记忆处处可见；项目记忆只在同一路径下可见。
    ///
    /// 参数:
    /// - `workspace`: 当前工作区路径；无工作区时为 None
    ///
    /// 返回:
    /// - 可见时为 true
    pub fn visible_in(&self, workspace: Option<&str>) -> bool {
        match self {
            Self::Global => true,
            Self::Project { path } => workspace.is_some_and(|current| paths_match(path, current)),
        }
    }
}

/// 比较两个工作区路径是否指向同一目录。
///
/// 只做尾部分隔符归一化，不解析符号链接：解析需要访问文件系统，
/// 而召回过滤在热路径上。
///
/// 参数:
/// - `left`: 记忆记录的路径
/// - `right`: 当前工作区路径
///
/// 返回:
/// - 指向同一目录时为 true
fn paths_match(left: &str, right: &str) -> bool {
    normalize(left) == normalize(right)
}

/// 去掉路径尾部的分隔符。
///
/// 参数:
/// - `value`: 原始路径
///
/// 返回:
/// - 归一化后的路径
fn normalize(value: &str) -> &str {
    let trimmed = value.trim_end_matches(['/', '\\']);
    if trimmed.is_empty() {
        value
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证空路径还原为全局作用域。
    #[test]
    fn blank_path_restores_to_global() {
        assert_eq!(MemoryScope::from_stored(None), MemoryScope::Global);
        assert_eq!(MemoryScope::from_stored(Some("  ")), MemoryScope::Global);
    }

    /// 验证全局记忆在任何工作区都可见。
    #[test]
    fn global_memories_are_visible_everywhere() {
        assert!(MemoryScope::Global.visible_in(Some("/home/a")));
        assert!(MemoryScope::Global.visible_in(None));
    }

    /// 验证项目记忆不会泄漏到其它工作区。
    #[test]
    fn project_memories_stay_in_their_workspace() {
        let scope = MemoryScope::from_stored(Some("/home/a"));
        assert!(scope.visible_in(Some("/home/a")));
        assert!(!scope.visible_in(Some("/home/b")));
        assert!(!scope.visible_in(None));
    }

    /// 验证尾部分隔符不影响匹配。
    #[test]
    fn trailing_separators_do_not_break_matching() {
        let scope = MemoryScope::from_stored(Some("/home/a/"));
        assert!(scope.visible_in(Some("/home/a")));
    }
}
