use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use crate::config::AppConfig;
use crate::memory::file_store::{render_index_injection_for, Frontmatter, MemoryEntry, MemoryScope, MemoryType};
use crate::memory::MemoryStore;
use axum::extract::{Path, Query, State};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;

/// 列表查询参数。
#[derive(Deserialize)]
struct ListQuery {
    limit: Option<usize>,
    /// 工作区标识；省略时用当前活动工作区
    workspace: Option<String>,
}

/// 写入请求体。
#[derive(Deserialize)]
struct RememberRequest {
    name: String,
    description: String,
    content: String,
    #[serde(default = "default_type")]
    memory_type: String,
    #[serde(default)]
    global: bool,
    /// 索引行里的一句话提示；留空时沿用摘要
    #[serde(default)]
    hook: String,
    /// 工作区标识；省略时用当前活动工作区
    #[serde(default)]
    workspace: Option<String>,
}

/// 逐出记录检索参数。
#[derive(Deserialize)]
struct SearchQuery {
    q: String,
    limit: Option<usize>,
}

/// 返回默认条目类型。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 类型标识
fn default_type() -> String {
    "project".to_string()
}

/// 记忆管理路由。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 路由表
pub(super) fn routes() -> Router<WebAppState> {
    Router::new()
        .route("/api/memory/stats", get(stats))
        .route("/api/memory/entries", get(list).post(remember))
        .route("/api/memory/entries/:name", delete(remove).get(show))
        .route("/api/memory/evicted", get(search_evicted))
        .route("/api/memory/index", get(index))
        .route("/api/memory/reset", post(reset))
}

/// 打开当前配置对应的记忆入口。
///
/// 参数:
/// - `state`: Web 应用状态
///
/// 返回:
/// - 记忆入口
fn store(state: &WebAppState) -> MemoryStore {
    let config = AppConfig::load_or_default(&state.paths).unwrap_or_default();
    MemoryStore::new(&config, &state.paths)
}

/// 解析记忆操作面向的工作区目录。
///
/// 项目记忆按工作区分目录存放，用服务进程的 cwd 会把别的工作区的记忆
/// 显示出来。显式指定优先，其次活动工作区，最后才退回 cwd。
///
/// 参数:
/// - `workspaces`: 工作区注册表
/// - `requested`: 请求里指定的工作区标识
///
/// 返回:
/// - 工作区目录；无法确定时为空
fn workspace_for(
    workspaces: &super::super::workspaces::WorkspaceManager,
    requested: Option<&str>,
) -> Option<PathBuf> {
    match requested {
        Some(id) => workspaces.get(id).ok().map(|info| info.path.into()),
        None => workspaces
            .active()
            .ok()
            .map(|info| info.path.into())
            .or_else(|| crate::runtime_cwd::current_dir().ok()),
    }
}

/// 返回记忆状态汇总。
async fn stats(
    State(state): State<WebAppState>,
    Query(query): Query<ListQuery>,
) -> WebResult<Json<Value>> {
    let store = store(&state);
    let workspace = workspace_for(&state.workspaces, query.workspace.as_deref());
    let summaries = store
        .notes(workspace.as_deref())
        .list()
        .map_err(WebError::from)?;
    let project = summaries
        .iter()
        .filter(|summary| summary.scope == MemoryScope::Project)
        .count();
    Ok(Json(json!({
        "ok": true,
        "notes_dir": store.notes_dir().display().to_string(),
        "memories": summaries.len(),
        "project_memories": project,
        "global_memories": summaries.len() - project,
        "evicted_turns": store.evicted_count().map_err(WebError::from)?,
        "storage": { "mode": "markdown_files" },
    })))
}

/// 列出记忆条目。
async fn list(
    State(state): State<WebAppState>,
    Query(query): Query<ListQuery>,
) -> WebResult<Json<Value>> {
    let store = store(&state);
    let workspace = workspace_for(&state.workspaces, query.workspace.as_deref());
    let mut entries: Vec<Value> = store
        .notes(workspace.as_deref())
        .list()
        .map_err(WebError::from)?
        .into_iter()
        .map(|summary| {
            json!({
                "name": summary.name,
                "description": summary.description,
                "type": summary.memory_type.as_str(),
                "scope": scope_label(summary.scope),
            })
        })
        .collect();
    entries.truncate(query.limit.unwrap_or(200));
    Ok(Json(json!({ "ok": true, "count": entries.len(), "entries": entries })))
}

/// 读取一条记忆的完整内容。
async fn show(
    State(state): State<WebAppState>,
    Path(name): Path<String>,
    Query(query): Query<ListQuery>,
) -> WebResult<Json<Value>> {
    let store = store(&state);
    let workspace = workspace_for(&state.workspaces, query.workspace.as_deref());
    let found = store
        .notes(workspace.as_deref())
        .load(&name)
        .map_err(WebError::from)?;
    let Some((entry, scope)) = found else {
        return Ok(Json(json!({ "found": false, "name": name })));
    };
    Ok(Json(json!({
        "found": true,
        "name": entry.front.name,
        "description": entry.front.description,
        "type": entry.front.memory_type.as_str(),
        "scope": scope_label(scope),
        "content": entry.body,
        "links": entry.links(),
    })))
}

/// 写入或更新一条记忆。
async fn remember(
    State(state): State<WebAppState>,
    Json(request): Json<RememberRequest>,
) -> WebResult<Json<Value>> {
    let memory_type = MemoryType::parse(&request.memory_type)
        .ok_or_else(|| WebError::from(anyhow::anyhow!("未知记忆类型：{}", request.memory_type)))?;
    let scope = if request.global {
        MemoryScope::Global
    } else {
        MemoryScope::Project
    };
    let entry = MemoryEntry {
        front: Frontmatter {
            name: request.name.clone(),
            description: request.description.clone(),
            memory_type,
        },
        body: request.content,
    };
    // 索引行提示缺省沿用摘要，与 write_memory 工具保持一致
    let hook = if request.hook.trim().is_empty() {
        request.description.clone()
    } else {
        request.hook
    };
    let store = store(&state);
    let workspace = workspace_for(&state.workspaces, request.workspace.as_deref());
    let library = store.notes(workspace.as_deref());
    // 必须在 save 之前判断是否已存在：写入同名条目即就地更新
    let updated = library.load(&request.name).map_err(WebError::from)?.is_some();
    let links = entry.links();
    library
        .save(scope, &entry, &hook)
        .map_err(WebError::from)?;
    let mut result = json!({
        "ok": true,
        "name": request.name,
        "scope": scope_label(scope),
        "updated": updated,
        "links": links,
    });
    // 缺理由不阻止写入，但要让界面提示补写
    if let Some(missing) = memory_type.missing_rationale(&entry.body) {
        result["note"] = json!(missing);
    }
    Ok(Json(result))
}

/// 删除一条记忆。
async fn remove(
    State(state): State<WebAppState>,
    Path(name): Path<String>,
    Query(query): Query<ListQuery>,
) -> WebResult<Json<Value>> {
    let store = store(&state);
    let workspace = workspace_for(&state.workspaces, query.workspace.as_deref());
    let deleted = store
        .notes(workspace.as_deref())
        .delete(&name)
        .map_err(WebError::from)?;
    Ok(Json(json!({ "deleted": deleted })))
}

/// 检索被压缩清出上下文的轮次。
async fn search_evicted(
    State(state): State<WebAppState>,
    Query(query): Query<SearchQuery>,
) -> WebResult<Json<Value>> {
    Ok(Json(
        store(&state)
            .search_evicted_context_readonly(&query.q, query.limit.unwrap_or(20))
            .map_err(WebError::from)?,
    ))
}

/// 返回每轮实际注入给模型的记忆索引。
///
/// 界面上看到的必须和模型看到的一致，否则「明明记过却没生效」无从排查。
async fn index(
    State(state): State<WebAppState>,
    Query(query): Query<ListQuery>,
) -> WebResult<Json<Value>> {
    let store = store(&state);
    let workspace = workspace_for(&state.workspaces, query.workspace.as_deref());
    let text = render_index_injection_for(
        store.notes_dir(),
        workspace.as_deref().and_then(|path| path.to_str()),
    );
    Ok(Json(json!({
        "ok": true,
        "injected": text.is_some(),
        "text": text.unwrap_or_default(),
    })))
}

/// 清空全部记忆。
async fn reset(State(state): State<WebAppState>) -> WebResult<Json<Value>> {
    store(&state).reset_all().map_err(WebError::from)?;
    Ok(Json(json!({ "ok": true })))
}

/// 返回作用域的展示标识。
///
/// 参数:
/// - `scope`: 作用域
///
/// 返回:
/// - 小写标识
fn scope_label(scope: MemoryScope) -> &'static str {
    match scope {
        MemoryScope::Global => "global",
        MemoryScope::Project => "project",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::file_store::MemoryEntry;
    use crate::paths::SaiPaths;

    /// 构造测试用路径集合。
    fn test_paths(root: &std::path::Path) -> SaiPaths {
        SaiPaths {
            config_dir: root.join("config"),
            config_file: root.join("config/config.jsonc"),
            secrets_file: root.join("config/secrets.jsonc"),
            skills_dir: root.join("config/skills"),
            data_dir: root.join("data"),
            cache_dir: root.join("cache"),
            state_dir: root.join("state"),
            pictures_dir: root.join("pictures"),
            fish_hook_file: root.join("fish/sai.fish"),
            bash_hook_file: root.join("shell/bash-hook.sh"),
            zsh_hook_file: root.join("shell/zsh-hook.zsh"),
            powershell_hook_file: root.join("shell/powershell-hook.ps1"),
        }
    }

    /// 在指定工作区里写一条项目记忆。
    fn write_project_memory(store: &MemoryStore, workspace: &std::path::Path, name: &str) {
        let entry = MemoryEntry {
            front: Frontmatter {
                name: name.to_string(),
                description: format!("{name} 的摘要"),
                memory_type: MemoryType::Project,
            },
            body: "正文".to_string(),
        };
        store
            .notes(Some(workspace))
            .save(MemoryScope::Project, &entry, &entry.front.description)
            .unwrap();
    }

    /// 工作区解析：显式指定优先于活动工作区。
    ///
    /// 项目记忆按工作区分目录存放，解析错就会把别的工作区的记忆显示出来。
    /// 构造管理器会把进程 cwd 切进活动工作区，测试结束必须切回，
    /// 否则临时目录一删，同进程其它测试的 `current_dir()` 全部 ENOENT。
    #[test]
    fn workspace_resolution_prefers_the_requested_workspace() {
        let previous_cwd = std::env::current_dir().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        let first = temp.path().join("first");
        let second = temp.path().join("second");
        std::fs::create_dir_all(&first).unwrap();
        std::fs::create_dir_all(&second).unwrap();

        let result = {
            let workspaces =
                super::super::super::workspaces::WorkspaceManager::new(&paths, Some(&first))
                    .unwrap();
            let second_info = workspaces.add(&second, None).unwrap();

            let active = workspace_for(&workspaces, None).unwrap();
            let asked = workspace_for(&workspaces, Some(&second_info.id)).unwrap();
            (active, asked)
        };
        std::env::set_current_dir(&previous_cwd).unwrap();

        assert_eq!(
            result.0,
            crate::platform::windows_path::canonicalize(&first).unwrap_or(first.clone())
        );
        assert_eq!(
            result.1,
            crate::platform::windows_path::canonicalize(&second).unwrap_or(second.clone())
        );
    }

    /// 项目记忆必须按工作区隔离：这是界面显示错工作区记忆的根因。
    #[test]
    fn project_memories_are_isolated_per_workspace() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        let first = temp.path().join("first");
        let second = temp.path().join("second");
        std::fs::create_dir_all(&first).unwrap();
        std::fs::create_dir_all(&second).unwrap();
        let store = MemoryStore::new(&AppConfig::default(), &paths);

        write_project_memory(&store, &first, "only-in-first");
        write_project_memory(&store, &second, "only-in-second");

        let names_in_first = store
            .notes(Some(&first))
            .list()
            .unwrap()
            .into_iter()
            .map(|summary| summary.name)
            .collect::<Vec<_>>();
        assert_eq!(names_in_first, vec!["only-in-first".to_string()]);

        let names_in_second = store
            .notes(Some(&second))
            .list()
            .unwrap()
            .into_iter()
            .map(|summary| summary.name)
            .collect::<Vec<_>>();
        assert_eq!(names_in_second, vec!["only-in-second".to_string()]);
    }

    /// 同名写入即就地更新，首次写入不算更新。
    #[test]
    fn saving_the_same_name_reports_updated() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        let workspace = temp.path().join("project");
        std::fs::create_dir_all(&workspace).unwrap();
        let store = MemoryStore::new(&AppConfig::default(), &paths);
        let library = store.notes(Some(&workspace));

        let entry = MemoryEntry {
            front: Frontmatter {
                name: "dup".to_string(),
                description: "摘要".to_string(),
                memory_type: MemoryType::Reference,
            },
            body: "正文".to_string(),
        };
        assert!(!library.load("dup").unwrap().is_some());
        library
            .save(MemoryScope::Project, &entry, "摘要")
            .unwrap();
        assert!(library.load("dup").unwrap().is_some());
    }
}
