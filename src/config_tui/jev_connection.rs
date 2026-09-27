//! Jev 接入选择、编辑与阻塞式连接测试。

use crate::config::{AppConfig, JevConnection, ModelEndpointConfig, ModelEndpointKind};
use crate::i18n::text as t;
use crate::jev::probe::{self, ProbeReport};
use anyhow::Result;
use std::io;

use super::form::{run_form, Field};
use super::ui::message;

/// 选择内置功能使用的 JEV 接入。
///
/// 参数:
/// - `stdout`: 终端标准输出
/// - `config`: 待更新应用配置
///
/// 返回:
/// - 表单退出结果
pub(super) fn select_connection(stdout: &mut io::Stdout, config: &mut AppConfig) -> Result<()> {
    let ids = config
        .model_endpoints
        .iter()
        .filter(|item| item.kind == ModelEndpointKind::Jev)
        .map(|item| item.id.clone())
        .collect::<Vec<_>>();
    let mut fields = vec![Field::new(t("JEV connection", "JEV 接入"), config.jev.endpoint_id.clone())
        .choices_owned(ids)
        .empty_choice_label(t(
            "Auto: first JEV connection, else official TypeSafe",
            "自动：第一条 JEV 接入，没有时使用 TypeSafe 官方",
        ))];
    if run_form(stdout, t(" JEV CONNECTION ", " JEV 接入 "), &mut fields)? {
        config.jev.endpoint_id = fields[0].value.trim().to_string();
    }
    Ok(())
}

/// 编辑当前生效 JEV 接入的地址、模型和密钥；尚无接入时保存会新建一条。
///
/// 参数:
/// - `stdout`: 终端标准输出
/// - `config`: 待更新应用配置
///
/// 返回:
/// - 表单退出结果
pub(super) fn edit_connection(stdout: &mut io::Stdout, config: &mut AppConfig) -> Result<()> {
    // 1. 以生效接入为初始值；没有接入时展示官方默认值
    let current = config.jev_endpoint().ok().flatten().cloned();
    let info = crate::config::jev_connection_info_for(current.as_ref());
    let mut fields = vec![
        Field::new(t("Request endpoint", "请求地址"), info.endpoint),
        Field::new(t("Model", "模型"), info.model),
        Field::new(
            t("API key, $env:NAME supported; empty reads TYPESAFE_API_KEY", "API Key，支持 $env:变量名；留空读取 TYPESAFE_API_KEY"),
            current.as_ref().map(|item| item.api_key.clone()).unwrap_or_default(),
        )
        .secret(),
    ];
    loop {
        if !run_form(stdout, t(" EDIT JEV CONNECTION ", " 编辑 JEV 接入 "), &mut fields)? {
            return Ok(());
        }
        // 2. 在副本上写入并整体校验，失败时就地提示
        let mut next = config.clone();
        apply_connection(&mut next, current.as_ref().map(|item| item.id.as_str()), &fields);
        match next.validate() {
            Ok(()) => {
                *config = next;
                return Ok(());
            }
            Err(error) => message(stdout, &format!("{}: {error}", t("Invalid input", "输入无效")))?,
        }
    }
}

/// 把接入表单写入配置：更新已有接入，或新建一条并设为生效接入。
///
/// 参数:
/// - `config`: 待更新配置副本
/// - `current_id`: 当前生效接入 id；为空表示新建
/// - `fields`: 地址、模型与密钥字段
///
/// 返回:
/// - 无
fn apply_connection(config: &mut AppConfig, current_id: Option<&str>, fields: &[Field]) {
    let endpoint = fields[0].value.trim().to_string();
    let model = fields[1].value.trim().to_string();
    let api_key = fields[2].value.trim().to_string();
    if let Some(item) = current_id.and_then(|id| {
        config
            .model_endpoints
            .iter_mut()
            .find(|item| item.kind == ModelEndpointKind::Jev && item.id == id)
    }) {
        item.endpoint = endpoint;
        item.model = model;
        item.api_key = api_key;
        item.api_keys.clear();
        return;
    }
    let id = unique_endpoint_id(&config.model_endpoints);
    config.model_endpoints.push(ModelEndpointConfig {
        id: id.clone(),
        kind: ModelEndpointKind::Jev,
        name: "TypeSafe Jev".to_string(),
        endpoint,
        protocol: "auto".to_string(),
        api_key,
        api_keys: Vec::new(),
        api_key_selected: None,
        api_key_balance: false,
        models: Vec::new(),
        model,
    });
    config.jev.endpoint_id = id;
}

/// 生成不与现有接入冲突的 id。
fn unique_endpoint_id(endpoints: &[ModelEndpointConfig]) -> String {
    (1..)
        .map(|index| format!("jev-{index}"))
        .find(|id| endpoints.iter().all(|item| &item.id != id))
        .unwrap_or_else(|| "jev".to_string())
}

/// 【Jev接入】【连接测试】在独立线程的运行时中执行测试，避免与外层异步运行时冲突。
///
/// 参数:
/// - `connection`: 已解析密钥的接入
///
/// 返回:
/// - 测试结果；运行时构造失败时以失败结果返回
pub(super) fn probe_blocking(connection: JevConnection) -> ProbeReport {
    let fallback = ProbeReport::failed(&connection, "failed to start probe runtime");
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map(|runtime| runtime.block_on(probe::probe(&connection)))
    })
    .join()
    .ok()
    .and_then(Result::ok)
    .unwrap_or(fallback)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(endpoint: &str, key: &str) -> Vec<Field> {
        vec![
            Field::new("endpoint", endpoint.to_string()),
            Field::new("model", "jev-latest".to_string()),
            Field::new("key", key.to_string()),
        ]
    }

    #[test]
    fn missing_connection_is_created_and_selected() {
        let mut config = AppConfig::default();
        apply_connection(&mut config, None, &fields(crate::config::JEV_OFFICIAL_ENDPOINT, "$env:MY_KEY"));
        assert_eq!(config.model_endpoints.len(), 1);
        assert_eq!(config.jev.endpoint_id, "jev-1");
        config.validate().unwrap();
    }

    #[test]
    fn existing_connection_is_updated_in_place() {
        let mut config = AppConfig::default();
        apply_connection(&mut config, None, &fields(crate::config::JEV_OFFICIAL_ENDPOINT, "a"));
        apply_connection(&mut config, Some("jev-1"), &fields("http://localhost:9087/decide", "b"));
        assert_eq!(config.model_endpoints.len(), 1);
        assert_eq!(config.model_endpoints[0].endpoint, "http://localhost:9087/decide");
        assert_eq!(config.model_endpoints[0].api_key, "b");
    }
}
