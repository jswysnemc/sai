//! 模型接入分区编辑器：提供 `j`/`k` 移动、`s` 键直存、`t` 键测速与脏草稿星标提示。

use super::*;
use crate::config_tui::form::{run_form, Field};
use crate::config_tui::input::UnsavedState;
use crate::config_tui::mouse::{ListMouseState, MenuInput};
use crate::config_tui::ui::draw_menu_interactive;
use crossterm::event::KeyModifiers;

/// 【模型接入】【草稿校验】在隔离配置副本上验证当前接入草稿的合法性。
///
/// 参数:
/// - `item`: 待校验的接入草稿
///
/// 返回:
/// - 合法返回 `Ok(())`；地址或协议非法时返回校验错误
pub(super) fn validate_draft(item: &ModelEndpointConfig) -> Result<()> {
    let mut check = AppConfig::default();
    check.model_endpoints.push(item.clone());
    check.validate()
}

/// 【模型接入】【脏检查】判断子表单编辑后的草稿相对进入时的基线是否发生改动。
///
/// 参数:
/// - `current`: 当前草稿
/// - `baseline`: 进入编辑器时的初始基线
///
/// 返回:
/// - 存在未提交改动时返回 true
pub(super) fn is_draft_dirty(
    current: &ModelEndpointConfig,
    baseline: &ModelEndpointConfig,
) -> bool {
    current != baseline
}

/// 【模型接入】【分区编辑】编辑连接与密钥，支持 `s` 直接校验保存并在存在改动时拦截静默返回。
///
/// 参数:
/// - `stdout`: 终端输出流
/// - `item`: 独立接入草稿
///
/// 返回:
/// - 用户确认保存后的草稿；放弃或无改动返回时为 `Ok(None)`
pub(super) fn edit(
    stdout: &mut io::Stdout,
    mut item: ModelEndpointConfig,
) -> Result<Option<ModelEndpointConfig>> {
    // 1. 【模型接入】【基线记录】保存进入编辑器时的快照，用于在按 q/Esc 时保护子表单改动
    let baseline = item.clone();
    let mut unsaved_guard = UnsavedState::default();
    let mut selected = 0usize;
    let mut mouse = ListMouseState::default();
    loop {
        let dirty = is_draft_dirty(&item, &baseline);
        unsaved_guard.set(dirty);

        let options = [
            t("Connection and model", "连接与模型"),
            t("Credentials", "密钥管理"),
            t("Fetch model catalog", "获取模型目录"),
            t("Test connection", "测试连接"),
            t("Save connection", "保存接入"),
        ]
        .iter()
        .enumerate()
        .map(|(index, label)| format!("{}  {label}", index + 1))
        .collect::<Vec<_>>();
        let details = vec![
            format!("{}\n{}\n{}", item.name, item.endpoint, item.model),
            t(
                "Multiple keys and environment references are supported.",
                "支持多密钥、选用密钥和环境变量引用。",
            )
            .into(),
            t(
                "Import models without blocking input. Jev accepts a model ID directly.",
                "在后台获取模型，Jev 可直接填写模型标识。",
            )
            .into(),
            if item.kind == ModelEndpointKind::ImageGeneration {
                t(
                    "Send one small image request using this draft.",
                    "使用此草稿发送一次小尺寸图片请求。",
                )
            } else {
                t(
                    "Send a minimal Jev request using this draft.",
                    "使用此草稿发送最小 Jev 请求。",
                )
            }
            .into(),
            t(
                "Apply to config draft; save config to write to disk.",
                "确认到配置草稿；保存配置后写入磁盘。",
            )
            .into(),
        ];
        let title = if dirty {
            t(" EDIT CONNECTION * ", " 编辑接入 * ")
        } else {
            t(" EDIT CONNECTION ", " 编辑接入 ")
        };
        let hitbox = draw_menu_interactive(
            stdout,
            title,
            &options,
            &details,
            selected,
            &mouse,
            &super::super::theme::help_line(&[
                ("1-5", t("jump", "跳转")),
                ("j/k", t("move", "移动")),
                ("t", t("test", "测试")),
                ("s", t("save", "保存")),
                ("q", t("cancel", "取消")),
            ]),
            &item.id,
        )?;
        let code = match hitbox.read(&mut mouse, selected, false)? {
            MenuInput::Redraw | MenuInput::Paste(_) => continue,
            MenuInput::Select(index) | MenuInput::RightClick(index) => {
                selected = index;
                continue;
            }
            MenuInput::Activate(index) => {
                selected = index;
                KeyCode::Enter
            }
            MenuInput::Action(code) => code,
            MenuInput::Key(key) => {
                if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    KeyCode::Char('s')
                } else {
                    key.code
                }
            }
        };
        match code {
            // 2. 【模型接入】【取消草稿】与供应商编辑器保持一致，按 q/Esc 明确放弃当前分区草稿修改
            KeyCode::Esc | KeyCode::Char('q') => return Ok(None),
            KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => selected = (selected + 1).min(4),
            KeyCode::Char(digit @ '1'..='5') => selected = digit as usize - '1' as usize,
            KeyCode::Char('t') => probe::test(stdout, &item)?,
            // 3. 【模型接入】【单键保存】支持按 s 直接校验草稿并保存返回上级菜单
            KeyCode::Char('s') => match validate_draft(&item) {
                Ok(()) => return Ok(Some(item)),
                Err(error) => message(stdout, &error.to_string())?,
            },
            KeyCode::Enter => match selected {
                0 => connection(stdout, &mut item)?,
                1 => super::super::provider_forms::edit_credentials(
                    stdout,
                    &mut item.api_key,
                    &mut item.api_keys,
                    &mut item.api_key_selected,
                    &mut item.api_key_balance,
                )?,
                2 => probe::catalog(stdout, &mut item)?,
                3 => probe::test(stdout, &item)?,
                _ => match validate_draft(&item) {
                    Ok(()) => return Ok(Some(item)),
                    Err(error) => message(stdout, &error.to_string())?,
                },
            },
            _ => {}
        }
    }
}

/// 【模型接入】【连接字段】保留完整请求路径；模型可从目录选取或手动填写。
///
/// 参数:
/// - `stdout`: 终端输出流
/// - `item`: 待修改草稿
///
/// 返回:
/// - 表单执行结果
fn connection(stdout: &mut io::Stdout, item: &mut ModelEndpointConfig) -> Result<()> {
    let mut models = vec![String::new()];
    models.extend(item.models.clone());
    if !models.contains(&item.model) {
        models.push(item.model.clone());
    }
    let refs = models.iter().map(String::as_str).collect::<Vec<_>>();
    let mut fields = vec![
        Field::new(t("Name", "显示名称"), item.name.clone()),
        Field::new(t("Full request URL", "完整请求地址"), item.endpoint.clone()),
        Field::new(
            t("Model ID (manual)", "模型标识（手动输入）"),
            item.model.clone(),
        ),
        Field::new(
            t(
                "Catalog model (optional override)",
                "目录模型（可选，覆盖手动输入）",
            ),
            String::new(),
        )
        .choices(&refs)
        .empty_choice_label(t("Use manual model", "使用手动模型")),
    ];
    if item.kind == ModelEndpointKind::ImageGeneration {
        fields.push(
            Field::new(t("Protocol", "协议"), item.protocol.clone()).choices(&[
                "auto",
                "openai-images",
                "gemini",
            ]),
        );
    }
    if run_form(
        stdout,
        t(" CONNECTION AND MODEL ", " 连接与模型 "),
        &mut fields,
    )? {
        item.name = fields[0].value.trim().into();
        item.endpoint = fields[1].value.trim().into();
        item.model = if fields[3].value.is_empty() {
            fields[2].value.trim().into()
        } else {
            fields[3].value.clone()
        };
        if let Some(protocol) = fields.get(4) {
            item.protocol = protocol.value.clone();
        }
    }
    Ok(())
}
