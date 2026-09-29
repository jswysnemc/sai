use super::*;

/// 【服务商配置】【连接编辑】编辑基本接入参数；取消时保持原配置。
/// 参数: stdout 为终端，provider 为草稿；返回: 表单执行结果
pub(super) fn edit(
    stdout: &mut io::Stdout,
    provider: &mut ProviderConfig,
    new_provider: bool,
) -> Result<()> {
    let mut fields = vec![
        Field::new(t("Config ID", "配置 ID"), provider.id.clone()),
        Field::new(t("Display name", "显示名称"), provider.display_name.clone()),
        Field::new(t("Base URL", "基础地址"), provider.base_url.clone()),
        Field::new(t("Protocol", "协议"), provider.protocol.clone()).choices(&[
            "auto",
            "openai-chat",
            "openai-responses",
            "anthropic",
        ]),
        Field::new(
            t("API key or $env:NAME", "密钥或 $env:NAME"),
            provider.api_key.clone().unwrap_or_default(),
        )
        .secret(),
        Field::boolean(t("Enabled", "启用"), provider.enabled),
    ];
    if !new_provider {
        fields[0].choices = vec![provider.id.clone()];
    }
    loop {
        if !run_form(stdout, t(" CONNECTION ", " 连接设置 "), &mut fields)? {
            return Ok(());
        }
        let url = values::normalize_base_url(&fields[2].value);
        if fields[0].value.trim().is_empty()
            || fields[1].value.trim().is_empty()
            || reqwest::Url::parse(&url).map_or(true, |url| {
                !matches!(url.scheme(), "http" | "https") || url.host_str().is_none()
            })
        {
            message(
                stdout,
                t(
                    "Enter an ID, name and HTTP(S) URL",
                    "请输入配置 ID、名称和 HTTP(S) 地址",
                ),
            )?;
            continue;
        }
        provider.id = fields[0].value.trim().into();
        provider.display_name = fields[1].value.trim().into();
        provider.base_url = url;
        provider.protocol = fields[3].value.clone();
        provider.api_key =
            Some(fields[4].value.trim().into()).filter(|key: &String| !key.is_empty());
        provider.enabled = parse_bool_field(&fields[5].value)?;
        return Ok(());
    }
}
