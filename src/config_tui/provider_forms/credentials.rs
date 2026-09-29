use super::*;
use crate::config::ProviderApiKey;

/// 【模型接入】【密钥编辑】统一编辑单密钥、多密钥和选中项；确认前不修改传入配置。
/// 参数: stdout 为终端，其余参数为密钥草稿；返回: 表单执行结果
pub(crate) fn edit_credentials(
    stdout: &mut io::Stdout,
    single: &mut String,
    keys: &mut Vec<ProviderApiKey>,
    selected: &mut Option<String>,
    balance: &mut bool,
) -> Result<()> {
    let selected_index = keys
        .iter()
        .position(|key| Some(&key.id) == selected.as_ref())
        .map(|index| (index + 1).to_string())
        .unwrap_or_default();
    let mut fields = vec![
        Field::new(
            t("API key or $env:NAME", "密钥或 $env:NAME"),
            single.clone(),
        )
        .secret(),
        Field::section(t("Multiple keys take priority", "多密钥非空时优先使用")),
        Field::textarea(
            t(
                "Keys: one per line, key | label",
                "密钥：每行一条，密钥 | 备注",
            ),
            keys::render_api_key_lines(keys),
        )
        .secret(),
        Field::new(
            t(
                "Selected key (1-based, blank = first)",
                "选用密钥序号（从 1 起，空为首个）",
            ),
            selected_index,
        ),
        Field::boolean(t("Balance keys", "密钥负载均衡"), *balance),
    ];
    loop {
        if !run_form(stdout, t(" CREDENTIALS ", " 密钥管理 "), &mut fields)? {
            return Ok(());
        }
        let parsed = keys::parse_api_key_lines(&fields[2].value, keys);
        let choice = keys::parse_selected_key(&fields[3].value, &parsed);
        if !fields[3].value.trim().is_empty() && choice.is_none() {
            message(
                stdout,
                t("Selected key is out of range", "选用密钥序号超出范围"),
            )?;
            continue;
        }
        *single = fields[0].value.trim().into();
        *keys = parsed;
        *selected = choice;
        *balance = parse_bool_field(&fields[4].value)?;
        return Ok(());
    }
}
