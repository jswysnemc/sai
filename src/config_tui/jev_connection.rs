//! Jev 接入选择、编辑与阻塞式连接测试。

use super::form::{run_form, Field};
use crate::config::{AppConfig, JevConnection, ModelEndpointKind};
use crate::i18n::text as t;
use crate::jev::probe::{self, ProbeReport};
use anyhow::Result;
use std::io;

/// 【Jev接入】【候选列表】构造包含首项空串自动模式与已配置 Jev 接入 ID 的候选列表。
///
/// 参数:
/// - `config`: 当前应用配置
///
/// 返回:
/// - 首项为空串（自动模式）的选项列表
fn connection_choice_ids(config: &AppConfig) -> Vec<String> {
    // 1. 【Jev接入】【自动选项】首项保留空串以匹配 empty_choice_label，确保用户可切回自动模式
    let mut ids = vec![String::new()];
    ids.extend(
        config
            .model_endpoints
            .iter()
            .filter(|item| item.kind == ModelEndpointKind::Jev)
            .map(|item| item.id.clone()),
    );
    ids
}

/// 【Jev接入】【选择生效项】选择内置功能使用的 JEV 接入，支持切回自动模式。
///
/// 参数:
/// - `stdout`: 终端标准输出
/// - `config`: 待更新应用配置
///
/// 返回:
/// - 表单退出结果
pub(super) fn select_connection(stdout: &mut io::Stdout, config: &mut AppConfig) -> Result<()> {
    let ids = connection_choice_ids(config);
    let mut fields = vec![Field::new(
        t("JEV connection", "JEV 接入"),
        config.jev.endpoint_id.clone(),
    )
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

/// 【Jev接入】【编辑当前项】编辑当前生效 JEV 接入的地址、模型和密钥；尚无接入时保存会新建一条。
///
/// 参数:
/// - `stdout`: 终端标准输出
/// - `config`: 待更新应用配置
///
/// 返回:
/// - 表单退出结果
pub(super) fn edit_connection(stdout: &mut io::Stdout, config: &mut AppConfig) -> Result<()> {
    super::model_endpoints::edit_active_jev(stdout, config)
}

/// 【Jev接入】【连接测试】在独立线程的运行时中执行测试，避免与外层异步运行时冲突。
///
/// 参数:
/// - `connection`: 已解析密钥的接入
///
/// 返回:
/// - 测试结果；运行时构造失败时以双语失败结果返回
pub(super) fn probe_blocking(connection: JevConnection) -> ProbeReport {
    let fallback = ProbeReport::failed(
        &connection,
        t("failed to start probe runtime", "无法启动连接测试运行时"),
    );
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
    use crate::config::ModelEndpointConfig;

    /// 【Jev接入】【自动选项回归测试】验证选项列表首项始终为空串自动模式，允许切回自动选择。
    #[test]
    fn connection_choices_start_with_auto_empty_option() {
        let mut config = AppConfig::default();
        config.model_endpoints.push(ModelEndpointConfig {
            id: "jev-1".into(),
            kind: ModelEndpointKind::Jev,
            name: "Primary".into(),
            endpoint: crate::config::JEV_OFFICIAL_ENDPOINT.into(),
            protocol: "auto".into(),
            model: crate::config::JEV_DEFAULT_MODEL.into(),
            api_key: String::new(),
            api_keys: Vec::new(),
            api_key_selected: None,
            api_key_balance: false,
            models: Vec::new(),
        });
        let choices = connection_choice_ids(&config);
        assert_eq!(choices, vec![String::new(), "jev-1".to_string()]);
    }
}
