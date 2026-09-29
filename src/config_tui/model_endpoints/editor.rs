use super::*;
use crate::config_tui::form::{run_form, Field};

/// 【模型接入】【分区编辑】编辑连接与密钥，支持目录导入及手动模型输入。
/// 参数: stdout 为终端，item 为独立草稿；返回: 保存后的草稿或取消
pub(super) fn edit(
    stdout: &mut io::Stdout,
    mut item: ModelEndpointConfig,
) -> Result<Option<ModelEndpointConfig>> {
    let mut selected = 0usize;
    loop {
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
        draw_menu_with_details(
            stdout,
            t(" EDIT CONNECTION ", " 编辑接入 "),
            &options,
            &details,
            selected,
            "1-5 · Enter · q/Esc",
            &item.id,
        )?;
        match read_key()? {
            KeyCode::Esc | KeyCode::Char('q') => return Ok(None),
            KeyCode::Up => selected = selected.saturating_sub(1),
            KeyCode::Down => selected = (selected + 1).min(4),
            KeyCode::Char(digit @ '1'..='5') => selected = digit as usize - '1' as usize,
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
                _ => {
                    let mut check = AppConfig::default();
                    check.model_endpoints.push(item.clone());
                    match check.validate() {
                        Ok(()) => return Ok(Some(item)),
                        Err(error) => message(stdout, &error.to_string())?,
                    }
                }
            },
            _ => {}
        }
    }
}

/// 【模型接入】【连接字段】保留完整请求路径；模型可从目录选取或手动填写。
/// 参数: stdout 为终端，item 为草稿；返回: 表单结果
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
