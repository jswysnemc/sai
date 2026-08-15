use crate::config::AppConfig;
use crate::memory::file_store::{FileMemoryLibrary, MemoryScope};
use crate::paths::SaiPaths;
use anyhow::{bail, Result};
use serde_json::Value;

/// 打开当前工作区对应的记忆库。
///
/// 工作区取运行时当前目录：项目记忆按目录隔离，取不到目录时退化为
/// 只有全局记忆，而不是把它们混进某个不相干的项目。
///
/// 参数:
/// - `config`: 应用配置，决定人格隔离的记忆目录
/// - `paths`: Sai 路径集合
///
/// 返回:
/// - 记忆库
pub(super) fn library(_config: &AppConfig, paths: &SaiPaths) -> FileMemoryLibrary {
    let workspace = crate::runtime_cwd::current_dir().ok();
    FileMemoryLibrary::new(&crate::memory::notes_dir(paths), workspace.as_deref())
}

/// 读取必填的字符串参数。
///
/// 参数:
/// - `args`: 工具入参
/// - `name`: 参数名
///
/// 返回:
/// - 去掉首尾空白的取值；缺失或为空时报错
pub(super) fn required_str<'a>(args: &'a Value, name: &str) -> Result<&'a str> {
    let value = args
        .get(name)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if value.is_empty() {
        bail!(
            "{}: {name}",
            crate::i18n::text("required argument missing", "缺少必需参数")
        );
    }
    Ok(value)
}

/// 读取可选的字符串参数。
///
/// 参数:
/// - `args`: 工具入参
/// - `name`: 参数名
///
/// 返回:
/// - 去掉首尾空白的取值；缺失时为空串
pub(super) fn optional_str<'a>(args: &'a Value, name: &str) -> &'a str {
    args.get(name)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
}

/// 解析作用域参数。
///
/// 默认落在项目：绝大多数记忆只在当前工作区成立，默认全局会让 A 项目的
/// 结论污染 B 项目。
///
/// 参数:
/// - `args`: 工具入参
///
/// 返回:
/// - 作用域
pub(super) fn parse_scope(args: &Value) -> MemoryScope {
    match optional_str(args, "scope").to_ascii_lowercase().as_str() {
        "global" => MemoryScope::Global,
        _ => MemoryScope::Project,
    }
}

/// 返回作用域的展示标识。
///
/// 参数:
/// - `scope`: 作用域
///
/// 返回:
/// - 小写标识
pub(super) fn scope_label(scope: MemoryScope) -> &'static str {
    match scope {
        MemoryScope::Global => "global",
        MemoryScope::Project => "project",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 验证缺少必填参数时报错。
    #[test]
    fn a_missing_required_argument_is_rejected() {
        assert!(required_str(&json!({}), "name").is_err());
        assert!(required_str(&json!({ "name": "   " }), "name").is_err());
    }

    /// 验证作用域默认落在项目。
    ///
    /// 默认全局会让某个项目的结论泄漏到其它项目。
    #[test]
    fn scope_defaults_to_project() {
        assert_eq!(parse_scope(&json!({})), MemoryScope::Project);
        assert_eq!(parse_scope(&json!({ "scope": "" })), MemoryScope::Project);
    }

    /// 验证显式指定全局作用域生效。
    #[test]
    fn global_scope_is_honored() {
        assert_eq!(parse_scope(&json!({ "scope": "Global" })), MemoryScope::Global);
    }

    /// 验证无法识别的作用域退回项目而不是报错。
    #[test]
    fn an_unknown_scope_falls_back_to_project() {
        assert_eq!(parse_scope(&json!({ "scope": "session" })), MemoryScope::Project);
    }
}
