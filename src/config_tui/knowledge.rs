use crate::config::AppConfig;
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use crate::tools::knowledge_base::KnowledgeBase;
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
    loop {
        let kb = KnowledgeBase::new(config.clone(), paths.clone())?;
        let files = kb.list().unwrap_or_default();
        let stats = kb.stats().ok();
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
        if files.is_empty() {
            options.push(format!("  ({})", t("no files yet", "暂无文件")));
        } else {
            for file in &files {
                options.push(format!("  {}  ({} B)", file.name, file.size_bytes));
            }
        }
        selected = selected.min(options.len().saturating_sub(1));
        let help = if status.is_empty() {
            t(
                "[Enter] add/delete [a] add [d] delete [r] refresh [q] back",
                "[Enter]添加/删除 [a]添加 [d]删除 [r]刷新 [q]返回",
            )
            .to_string()
        } else {
            status.clone()
        };
        let title = format!("{} · {}", t(" KNOWLEDGE BASE ", " 知识库管理 "), summary);
        draw_menu(stdout, &title, &options, selected, &help)?;

        match read_key()? {
            KeyCode::Esc | KeyCode::Char('q') => return Ok(()),
            KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => selected = (selected + 1).min(options.len() - 1),
            KeyCode::Char('r') => {
                status = t("refreshed", "已刷新").to_string();
            }
            KeyCode::Char('a') => match add_path(stdout, paths, config) {
                Ok(message) => status = message,
                Err(err) => status = err.to_string(),
            },
            KeyCode::Enter if selected == 0 => match add_path(stdout, paths, config) {
                Ok(message) => status = message,
                Err(err) => status = err.to_string(),
            },
            KeyCode::Enter if selected == 1 => match clear_all(stdout, paths, config) {
                Ok(message) => status = message,
                Err(err) => status = err.to_string(),
            },
            KeyCode::Char('d') | KeyCode::Enter if selected >= 2 && !files.is_empty() => {
                let index = selected - 2;
                if let Some(file) = files.get(index) {
                    match remove_one(paths, config, &file.name) {
                        Ok(message) => {
                            status = message;
                            selected = selected.saturating_sub(1).max(2);
                        }
                        Err(err) => status = err.to_string(),
                    }
                }
            }
            _ => {}
        }
    }
}

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
    let kb = KnowledgeBase::new(config.clone(), paths.clone())?;
    let added = block_on(kb.add_path(&path))?;
    Ok(format!("{}: {}", t("added", "已添加"), added.len()))
}

fn remove_one(paths: &SaiPaths, config: &AppConfig, name: &str) -> Result<String> {
    let kb = KnowledgeBase::new(config.clone(), paths.clone())?;
    kb.remove(name)?;
    Ok(format!("{} {name}", t("removed", "已移除")))
}

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
    let kb = KnowledgeBase::new(config.clone(), paths.clone())?;
    let files = kb.list()?;
    let count = files.len();
    for file in files {
        kb.remove(&file.name)?;
    }
    Ok(format!("{}: {}", t("cleared files", "已清空文件数"), count))
}

fn block_on<F: std::future::Future>(future: F) -> F::Output {
    tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(future))
}
