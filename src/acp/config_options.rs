use agent_client_protocol::schema::v1::{
    SessionConfigKind, SessionConfigOption, SessionConfigOptionCategory, SessionConfigOptionValue,
    SessionConfigSelectOptions, SetSessionConfigOptionRequest, SetSessionConfigOptionResponse,
};
use anyhow::{bail, Context, Result};
use serde_json::Value;

/// 当前 ACP 会话公开的配置项。
#[derive(Debug, Clone, Default)]
pub(crate) struct AcpConfigOptions {
    options: Vec<SessionConfigOption>,
}

impl AcpConfigOptions {
    /// 替换 agent 返回的完整配置状态。
    ///
    /// 参数:
    /// - `options`: session setup 或配置更新返回的完整配置项
    ///
    /// 返回:
    /// - 无
    pub(crate) fn replace(&mut self, options: Option<Vec<SessionConfigOption>>) {
        if let Some(options) = options {
            self.options = options;
        }
    }

    /// 按 Sai 配置依次下发模型、权限模式、思考等级和任意配置项。
    ///
    /// 参数:
    /// - `transport`: ACP 传输
    /// - `session_id`: 当前 ACP 会话标识
    /// - `config`: 外部内核配置
    ///
    /// 返回:
    /// - 配置更新结果
    pub(crate) async fn apply_configured_values(
        &mut self,
        transport: &super::transport::AcpTransport,
        session_id: &str,
        config: &crate::config::AcpEngineConfig,
    ) -> Result<()> {
        let mut requested = config.config_options.clone();
        insert_category_value(&mut requested, &self.options, "model", &config.model);
        insert_category_value(
            &mut requested,
            &self.options,
            "mode",
            &config.permission_mode,
        );
        insert_category_value(
            &mut requested,
            &self.options,
            "thought_level",
            &config.thought_level,
        );
        for (config_id, value) in requested {
            let option = self
                .options
                .iter()
                .find(|option| option.id.to_string() == config_id)
                .with_context(|| format!("ACP agent does not expose config option: {config_id}"))?
                .clone();
            self.set_value(transport, session_id, &option, value)
                .await?;
        }
        Ok(())
    }

    /// 设置一个已经由 agent 公布的配置项。
    ///
    /// 参数:
    /// - `transport`: ACP 传输
    /// - `session_id`: 当前会话标识
    /// - `option`: agent 公布的配置项
    /// - `value`: 待设置的字符串或布尔值
    ///
    /// 返回:
    /// - agent 返回的完整配置状态
    async fn set_value(
        &mut self,
        transport: &super::transport::AcpTransport,
        session_id: &str,
        option: &SessionConfigOption,
        value: Value,
    ) -> Result<()> {
        let request = SetSessionConfigOptionRequest::new(
            session_id.to_string(),
            option.id.clone(),
            config_value(option, value)?,
        );
        let response = transport
            .request("session/set_config_option", super::sdk::to_value(&request)?)
            .await?;
        let response: SetSessionConfigOptionResponse =
            super::sdk::from_value(response, "session/set_config_option response")?;
        self.options = response.config_options;
        Ok(())
    }

    /// 返回当前完整配置项。
    ///
    /// 返回:
    /// - agent 最近公布的标准配置项
    pub(crate) fn options(&self) -> &[SessionConfigOption] {
        &self.options
    }

    /// 返回可供界面展示的原始标准配置项。
    ///
    /// 返回:
    /// - 官方 SDK 配置项的 JSON 表示
    #[allow(dead_code)]
    pub(crate) fn as_json(&self) -> Value {
        serde_json::to_value(&self.options).unwrap_or_else(|_| Value::Array(Vec::new()))
    }
}

/// 按语义类别查找配置 id，并加入待设置值。
///
/// 参数:
/// - `requested`: 待设置的 id 到值映射
/// - `options`: agent 公布的配置项
/// - `category`: ACP 标准类别
/// - `value`: Sai 配置值
///
/// 返回:
/// - 无
fn insert_category_value(
    requested: &mut std::collections::BTreeMap<String, Value>,
    options: &[SessionConfigOption],
    category: &str,
    value: &str,
) {
    if value.trim().is_empty() {
        return;
    }
    let option = options.iter().find(|option| {
        matches!(
            (&option.category, category),
            (Some(SessionConfigOptionCategory::Model), "model")
                | (Some(SessionConfigOptionCategory::Mode), "mode")
                | (
                    Some(SessionConfigOptionCategory::ThoughtLevel),
                    "thought_level"
                )
        )
    });
    if let Some(option) = option {
        requested
            .entry(option.id.to_string())
            .or_insert_with(|| Value::String(value.to_string()));
    }
}

/// 把配置文件值转换成 ACP SDK 配置值。
///
/// 参数:
/// - `value`: 字符串选择值或布尔值
///
/// 返回:
/// - 标准 ACP 配置值
fn config_value(option: &SessionConfigOption, value: Value) -> Result<SessionConfigOptionValue> {
    match (&option.kind, value) {
        (SessionConfigKind::Select(select), Value::String(value)) => {
            if !select_values(&select.options).any(|candidate| candidate == value) {
                bail!(
                    "ACP config option {} does not accept value {value}",
                    option.id
                );
            }
            Ok(SessionConfigOptionValue::value_id(value))
        }
        (SessionConfigKind::Boolean(_), Value::Bool(value)) => {
            Ok(SessionConfigOptionValue::boolean(value))
        }
        (SessionConfigKind::Select(_), _) => {
            bail!("ACP config option {} requires a string value", option.id)
        }
        (SessionConfigKind::Boolean(_), _) => {
            bail!("ACP config option {} requires a boolean value", option.id)
        }
        _ => bail!("ACP config option {} uses an unsupported type", option.id),
    }
}

/// 遍历选择型配置项中的扁平或分组选项值。
///
/// 参数:
/// - `options`: ACP 选择项集合
///
/// 返回:
/// - 所有可选值的迭代器
fn select_values(options: &SessionConfigSelectOptions) -> Box<dyn Iterator<Item = String> + '_> {
    match options {
        SessionConfigSelectOptions::Ungrouped(options) => {
            Box::new(options.iter().map(|option| option.value.to_string()))
        }
        SessionConfigSelectOptions::Grouped(groups) => Box::new(
            groups
                .iter()
                .flat_map(|group| group.options.iter())
                .map(|option| option.value.to_string()),
        ),
        _ => Box::new(std::iter::empty()),
    }
}

#[cfg(test)]
mod tests {
    use super::config_value;
    use agent_client_protocol::schema::v1::{SessionConfigOption, SessionConfigSelectOption};

    /// 配置项只接受协议支持的字符串和布尔值。
    #[test]
    fn accepts_standard_config_value_shapes() {
        let select = SessionConfigOption::select(
            "thought",
            "Thought",
            "low",
            vec![
                SessionConfigSelectOption::new("low", "Low"),
                SessionConfigSelectOption::new("high", "High"),
            ],
        );
        let boolean = SessionConfigOption::boolean("enabled", "Enabled", false);
        assert!(config_value(&select, serde_json::json!("high")).is_ok());
        assert!(config_value(&select, serde_json::json!("missing")).is_err());
        assert!(config_value(&boolean, serde_json::json!(true)).is_ok());
        assert!(config_value(&boolean, serde_json::json!(3)).is_err());
    }
}
