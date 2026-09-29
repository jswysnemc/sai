mod editor;
mod operations;
mod probe;
#[cfg(test)]
mod tests;

use super::{
    input::read_key,
    ui::{confirm_delete, draw_menu_with_details, message},
};
use crate::config::{AppConfig, ModelEndpointConfig, ModelEndpointKind};
use crate::i18n::text as t;
use anyhow::Result;
use crossterm::event::KeyCode;
use std::io;

/// 【模型接入】【列表管理】管理 Jev 与生图接入，修改保留在配置会话中。
/// 参数: stdout 为终端，config 为会话草稿，kind 为接入类型；返回: 菜单结果
pub(super) fn run(
    stdout: &mut io::Stdout,
    config: &mut AppConfig,
    kind: ModelEndpointKind,
) -> Result<()> {
    let mut selected = 0usize;
    loop {
        let endpoints = config
            .model_endpoints
            .iter()
            .filter(|item| item.kind == kind)
            .cloned()
            .collect::<Vec<_>>();
        selected = selected.min(endpoints.len());
        let active = operations::active_id(config, kind);
        let mut options = endpoints
            .iter()
            .map(|item| {
                format!(
                    "{} {}",
                    if Some(item.id.as_str()) == active {
                        "*"
                    } else {
                        " "
                    },
                    item.name
                )
            })
            .collect::<Vec<_>>();
        options.push(t("Add connection", "新增接入").into());
        let mut details = endpoints
            .iter()
            .map(|item| {
                format!(
                    "{}\n{}\n{} · {}",
                    item.id, item.endpoint, item.protocol, item.model
                )
            })
            .collect::<Vec<_>>();
        details.push(t("Configure an independent endpoint, model and credentials. * marks the current default.", "配置独立请求地址、模型与密钥。* 表示当前默认接入。").into());
        draw_menu_with_details(
            stdout,
            if kind == ModelEndpointKind::Jev {
                " JEV "
            } else {
                t(" IMAGE MODELS ", " 生图模型 ")
            },
            &options,
            &details,
            selected,
            &super::theme::help_line(&[
                ("Enter", t("edit", "编辑")),
                ("a", t("add", "新增")),
                ("d", t("delete", "删除")),
                ("Space", t("use", "启用")),
                ("q", t("back", "返回")),
            ]),
            "",
        )?;
        match read_key()? {
            KeyCode::Esc | KeyCode::Char('q') => return Ok(()),
            KeyCode::Up => selected = selected.saturating_sub(1),
            KeyCode::Down => selected = (selected + 1).min(endpoints.len()),
            KeyCode::Char(' ') if selected < endpoints.len() => {
                operations::activate(config, &endpoints[selected].id)
            }
            KeyCode::Char('d') if selected < endpoints.len() => {
                let item = &endpoints[selected];
                if confirm_delete(
                    stdout,
                    t(" DELETE CONNECTION ", " 删除接入 "),
                    &item.name,
                    t(
                        "The next connection becomes the default when needed.",
                        "删除默认接入后，使用下一条可用接入。",
                    ),
                )? {
                    operations::remove(config, &item.id);
                }
            }
            key @ (KeyCode::Enter | KeyCode::Char('a')) => {
                let current = if key == KeyCode::Char('a') {
                    None
                } else {
                    endpoints.get(selected)
                };
                let item = current
                    .cloned()
                    .unwrap_or_else(|| operations::new_endpoint(config, kind));
                if let Some(updated) = editor::edit(stdout, item)? {
                    if let Err(error) = operations::save(config, updated) {
                        message(stdout, &error.to_string())?;
                    }
                }
            }
            _ => {}
        }
    }
}

/// 【模型接入】【分类入口】选择接入类型或进入 Jev 功能设置。
/// 参数: stdout 为终端，config 为草稿；返回: 菜单结果
pub(super) fn menu(stdout: &mut io::Stdout, config: &mut AppConfig) -> Result<()> {
    let mut selected = 0usize;
    loop {
        super::ui::draw_menu(
            stdout,
            t(" MODEL CONNECTIONS ", " 模型接入 "),
            &[
                "1  Jev".into(),
                t("2  Image models", "2  生图模型").into(),
                t("3  Jev routing and audit", "3  Jev 路由与审核").into(),
            ],
            selected,
            "1-3 · Enter · q/Esc",
        )?;
        match read_key()? {
            KeyCode::Esc | KeyCode::Char('q') => return Ok(()),
            KeyCode::Up => selected = selected.saturating_sub(1),
            KeyCode::Down => selected = (selected + 1).min(2),
            KeyCode::Char(digit @ '1'..='3') => selected = digit as usize - '1' as usize,
            KeyCode::Enter => match selected {
                0 => run(stdout, config, ModelEndpointKind::Jev)?,
                1 => run(stdout, config, ModelEndpointKind::ImageGeneration)?,
                _ => super::jev::edit_jev(stdout, config)?,
            },
            _ => {}
        }
    }
}

/// 【Jev接入】【统一编辑】编辑生效接入并保留多密钥、模型目录等已有配置。
/// 参数: stdout 为终端，config 为草稿；返回: 编辑结果
pub(super) fn edit_active_jev(stdout: &mut io::Stdout, config: &mut AppConfig) -> Result<()> {
    let current = config
        .jev_endpoint()?
        .cloned()
        .unwrap_or_else(|| operations::new_endpoint(config, ModelEndpointKind::Jev));
    if let Some(item) = editor::edit(stdout, current)? {
        let id = item.id.clone();
        operations::save(config, item)?;
        operations::activate(config, &id);
    }
    Ok(())
}
