//! Jev 接入选择、编辑与阻塞式连接测试。

use super::form::{run_form, Field};
use crate::config::{AppConfig, JevConnection, ModelEndpointKind};
use crate::i18n::text as t;
use crate::jev::probe::{self, ProbeReport};
use anyhow::Result;
use std::io;

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

/// 编辑当前生效 JEV 接入的地址、模型和密钥；尚无接入时保存会新建一条。
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
