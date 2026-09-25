use crate::config::AppConfig;
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use crate::plugins::knowledge_view::{self, FileRecord};
use anyhow::{bail, Result};
use crossterm::event::KeyCode;
use std::io;
use std::path::PathBuf;

use super::form::{run_form, Field};
use super::input::read_key;
use super::ui::draw_menu;

/// 在配置界面中管理本地知识库文件。
///
/// 参数:
/// - `stdout`: 终端输出
/// - `paths`: Sai 路径
/// - `config`: 当前配置（读取知识库插件设置）
///
/// 返回:
/// - 成功或用户返回
pub(crate) fn edit_knowledge_base(
    stdout: &mut io::Stdout,
    paths: &SaiPaths,
    config: &AppConfig,
) -> Result<()> {
    let mut selected = 0usize;
    let mut status = String::new();
    let mut search = super::search::ListSearch::default();
    // 1. 【知识库界面】【按需刷新】仅在进入或修改时查询插件，光标移动不重新加载数据
    let mut files = Vec::new();
    let mut stats = None;
    reload_knowledge(paths, config, &mut files, &mut stats);
    loop {
        let summary = stats
            .as_ref()
            .and_then(|value| {
                let files = value.get("files")?.as_u64()?;
                let size = value.get("total_size_kb")?.as_f64()?;
                Some(format!(
                    "{}: {files}  {} {:.1} KB  {} {}",
                    t("files", "文件"),
                    t("size", "大小"),
                    size,
                    t("dir", "目录"),
                    value
                        .get("files_dir")
                        .and_then(|v| v.as_str())
                        .unwrap_or("-")
                ))
            })
            .unwrap_or_else(|| {
                t("knowledge base empty or unavailable", "知识库为空或不可用").to_string()
            });

        let mut options = Vec::with_capacity(files.len().max(1) + 2);
        options.push(format!(
            "+ {}",
            t("Add file or directory", "添加文件或目录")
        ));
        options.push(format!(
            "! {}",
            t("Clear all knowledge base files", "清空全部知识库文件")
        ));
        let shown = files
            .iter()
            .enumerate()
            .filter(|(_, file)| search.matches(&file.name))
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if files.is_empty() {
            options.push(format!("  ({})", t("no files yet", "暂无文件")));
        } else if shown.is_empty() {
            options.push(format!("  ({})", t("no matches", "没有匹配")));
        } else {
            for index in &shown {
                let file = &files[*index];
                options.push(format!("  {}  ({} B)", file.name, file.size_bytes));
            }
        }
        selected = selected.min(options.len().saturating_sub(1));
        let help = search.help().unwrap_or_else(|| {
            if status.is_empty() {
                // Enter 只作用于上方两个操作项，文件行上不做事，避免「回车即删除」
                super::theme::help_line(&[
                    ("/", t("search", "搜索")),
                    ("a", t("add", "添加")),
                    ("d", t("delete file", "删除文件")),
                    ("r", t("refresh", "刷新")),
                    ("q", t("back", "返回")),
                ])
            } else {
                status.clone()
            }
        });
        let title = format!("{} · {}", t(" KNOWLEDGE BASE ", " 知识库管理 "), summary);
        draw_menu(stdout, &title, &options, selected, &help)?;

        let key = read_key()?;
        match search.handle(key) {
            super::search::SearchEffect::Updated => {
                selected = 0;
                continue;
            }
            super::search::SearchEffect::Closed => continue,
            super::search::SearchEffect::Passthrough => {}
        }
        let file_at = selected
            .checked_sub(2)
            .filter(|_| !shown.is_empty() || files.is_empty())
            .and_then(|index| shown.get(index).copied());
        match key {
            KeyCode::Esc | KeyCode::Char('q') if !search.editing => return Ok(()),
            KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => selected = (selected + 1).min(options.len() - 1),
            KeyCode::Char('r') => {
                reload_knowledge(paths, config, &mut files, &mut stats);
                status = t("refreshed", "已刷新").to_string();
            }
            KeyCode::Char('a') => {
                match add_path(stdout, paths, config) {
                    Ok(message) => status = message,
                    Err(err) => status = err.to_string(),
                }
                reload_knowledge(paths, config, &mut files, &mut stats);
            }
            KeyCode::Enter if selected == 0 => {
                match add_path(stdout, paths, config) {
                    Ok(message) => status = message,
                    Err(err) => status = err.to_string(),
                }
                reload_knowledge(paths, config, &mut files, &mut stats);
            }
            KeyCode::Enter if selected == 1 => {
                match clear_all(stdout, paths, config) {
                    Ok(message) => status = message,
                    Err(err) => status = err.to_string(),
                }
                reload_knowledge(paths, config, &mut files, &mut stats);
            }
            // 文件行上只认 d：Enter 在这里删除文件与其余界面「Enter 打开/执行」的
            // 语义冲突，且删除同时丢弃已索引向量，无法恢复
            KeyCode::Char('d') if !files.is_empty() => {
                let Some(index) = file_at else {
                    continue;
                };
                if let Some(file) = files.get(index) {
                    let name = file.name.clone();
                    match super::ui::confirm_delete(
                        stdout,
                        &t(" DELETE FILE ", " 删除文件 "),
                        &name,
                        &t(
                            "Removes the file and its indexed vectors from the knowledge base.",
                            "同时删除文件及其已索引向量，无法恢复。",
                        ),
                    ) {
                        Ok(true) => match remove_one(paths, config, &name) {
                            Ok(message) => {
                                status = message;
                                selected = selected.saturating_sub(1).max(2);
                                reload_knowledge(paths, config, &mut files, &mut stats);
                            }
                            Err(err) => status = err.to_string(),
                        },
                        Ok(false) => status = t("Delete cancelled", "已取消删除").to_string(),
                        Err(err) => status = err.to_string(),
                    }
                }
            }
            _ => {}
        }
    }
}

/// 重新读取知识库文件列表与统计。
///
/// 参数:
/// - `paths`: 应用目录
/// - `config`: 当前配置
/// - `files`: 待刷新的文件列表
/// - `stats`: 待刷新的统计信息
fn reload_knowledge(
    paths: &SaiPaths,
    config: &AppConfig,
    files: &mut Vec<FileRecord>,
    stats: &mut Option<serde_json::Value>,
) {
    *files = block_on(knowledge_view::list(paths, config)).unwrap_or_default();
    *stats = block_on(knowledge_view::stats(paths, config)).ok();
}

/// 【知识库界面】【输入导入】参数为输出、目录和配置；返回成功导入数量说明
fn add_path(stdout: &mut io::Stdout, paths: &SaiPaths, config: &AppConfig) -> Result<String> {
    let mut fields = [Field::new(
        t("Path to file or directory", "文件或目录路径"),
        String::new(),
    )];
    if !run_form(stdout, t(" ADD KNOWLEDGE ", " 添加知识库 "), &mut fields)? {
        return Ok(t("cancelled", "已取消").to_string());
    }
    let path = fields[0].value.trim();
    if path.is_empty() {
        bail!("{}", t("path is required", "路径不能为空"));
    }
    let path = PathBuf::from(path);
    if !path.exists() {
        bail!("{}: {}", t("path not found", "路径不存在"), path.display());
    }
    let added = block_on(knowledge_view::add(paths, config, &path))?;
    Ok(format!("{}: {}", t("added", "已添加"), added))
}

/// 【知识库界面】【单项删除】参数为应用目录、配置和已确认名称；返回结果说明
fn remove_one(paths: &SaiPaths, config: &AppConfig, name: &str) -> Result<String> {
    block_on(knowledge_view::remove(paths, config, name))?;
    Ok(format!("{} {name}", t("removed", "已移除")))
}

/// 【知识库界面】【确认清空】参数为输出、目录和配置；返回实际删除数量说明
fn clear_all(stdout: &mut io::Stdout, paths: &SaiPaths, config: &AppConfig) -> Result<String> {
    let mut fields = [Field::boolean(
        t(
            "Confirm clear all knowledge base files",
            "确认清空全部知识库文件",
        ),
        false,
    )];
    if !run_form(
        stdout,
        t(" CLEAR KNOWLEDGE BASE ", " 清空知识库 "),
        &mut fields,
    )? {
        return Ok(t("cancelled", "已取消").to_string());
    }
    if fields[0].value.trim() != "true" {
        return Ok(t("cancelled", "已取消").to_string());
    }
    let files = block_on(knowledge_view::list(paths, config))?;
    let count = files.len();
    for file in files {
        block_on(knowledge_view::remove(paths, config, &file.name))?;
    }
    Ok(format!("{}: {}", t("cleared files", "已清空文件数"), count))
}

/// 【知识库界面】【同步桥接】参数为插件 Future；返回完成结果，终端交互仍保留同步调用方式
fn block_on<F: std::future::Future>(future: F) -> F::Output {
    tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(future))
}
