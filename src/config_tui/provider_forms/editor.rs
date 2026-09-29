use super::*;
use crate::config_tui::ui::draw_menu_with_details;
use crossterm::event::KeyCode;

/// 【服务商配置】【分区编辑】管理独立草稿，保存返回新配置，取消丢弃全部分区修改。
/// 参数: stdout 为终端，provider 为原配置；返回: 已确认草稿或空
pub(crate) fn edit_provider_form(
    stdout: &mut io::Stdout,
    mut provider: ProviderConfig,
    new_provider: bool,
) -> Result<Option<ProviderConfig>> {
    let mut selected = 0;
    loop {
        let labels = [
            t("Connection", "连接设置"),
            t("Credentials", "密钥管理"),
            t("Advanced request settings", "高级请求设置"),
            t("Test and import models", "测试连接并导入模型"),
            t("Save provider", "保存供应商"),
        ];
        let options = labels
            .iter()
            .enumerate()
            .map(|(index, label)| format!("{}  {label}", index + 1))
            .collect::<Vec<_>>();
        let details = vec![
            format!("{}\n{}\n{}", provider.display_name, provider.base_url, provider.protocol),
            t("Manage a single key or a key pool. Values stay masked; environment references are supported.", "管理单密钥或密钥池，默认隐藏密钥，支持环境变量引用。").into(),
            t("Timeout, reasoning, request body and headers.", "超时、思考参数、自定义请求体与请求头。").into(),
            t("Fetch the catalog using this draft. Esc leaves the test; local models remain available on failure.", "使用当前草稿获取模型目录。Esc 返回，失败时仍可手动配置模型。").into(),
            t("Apply this draft. Save and exit config to write it to disk.", "确认此草稿；在配置主菜单保存退出后写入磁盘。").into(),
        ];
        draw_menu_with_details(
            stdout,
            t(" EDIT PROVIDER ", " 编辑供应商 "),
            &options,
            &details,
            selected,
            "1-5 · Enter · q/Esc",
            "",
        )?;
        match read_key()? {
            KeyCode::Esc | KeyCode::Char('q') => return Ok(None),
            KeyCode::Up => selected = selected.saturating_sub(1),
            KeyCode::Down => selected = (selected + 1).min(4),
            KeyCode::Char(digit @ '1'..='5') => selected = digit as usize - '1' as usize,
            KeyCode::Enter => match selected {
                0 => connection::edit(stdout, &mut provider, new_provider)?,
                1 => {
                    let mut single = provider.api_key.clone().unwrap_or_default();
                    edit_credentials(
                        stdout,
                        &mut single,
                        &mut provider.api_keys,
                        &mut provider.api_key_selected,
                        &mut provider.api_key_balance,
                    )?;
                    provider.api_key = Some(single).filter(|value| !value.is_empty());
                }
                2 => advanced::edit(stdout, &mut provider)?,
                3 => {
                    let draft = provider.clone();
                    if let Some(result) = crate::config_tui::background::run(
                        stdout,
                        t(" MODEL CONNECTION TEST ", " 模型连接测试 "),
                        move || super::super::provider_fetch::fetch_models(&draft),
                    )? {
                        match result {
                            Ok(result) => {
                                for model in result.models {
                                    if !provider.models.contains(&model) {
                                        provider.models.push(model);
                                    }
                                }
                                message(
                                    stdout,
                                    t(
                                        "Connection succeeded; models imported",
                                        "连接成功，已导入模型",
                                    ),
                                )?;
                            }
                            Err(error) => message(stdout, &error)?,
                        }
                    }
                }
                _ => return Ok(Some(provider)),
            },
            _ => {}
        }
    }
}
