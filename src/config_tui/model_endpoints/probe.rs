use super::*;
use crate::config_tui::background;

/// 【模型接入】【目录导入】后台请求模型目录，取消或失败时不修改现有列表。
/// 参数: stdout 为终端，item 为接入草稿；返回: 交互结果
pub(super) fn catalog(stdout: &mut io::Stdout, item: &mut ModelEndpointConfig) -> Result<()> {
    if item.kind == ModelEndpointKind::Jev {
        message(
            stdout,
            t(
                "Jev uses a model ID, such as jev-latest; edit it in Connection and model.",
                "Jev 使用模型标识，例如 jev-latest；请在连接与模型中填写。",
            ),
        )?;
        return Ok(());
    }
    let draft = item.clone();
    let result = background::run(stdout, t(" MODEL CATALOG ", " 模型目录 "), move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?
            .block_on(crate::web::services::model_endpoint_models::request_models(
                &draft,
            ))
    })?;
    if let Some(result) = result {
        match result {
            Ok(models) => {
                item.models = models;
                message(
                    stdout,
                    &format!(
                        "{}: {}",
                        t("Models imported", "已导入模型"),
                        item.models.len()
                    ),
                )?;
            }
            Err(error) => message(stdout, &error)?,
        }
    }
    Ok(())
}

/// 【模型接入】【连接测试】调用实际 Jev 或生图链路，工作线程隔离运行时。
/// 参数: stdout 为终端，item 为接入草稿；返回: 交互结果
pub(super) fn test(stdout: &mut io::Stdout, item: &ModelEndpointConfig) -> Result<()> {
    let draft = item.clone();
    let result = background::run(
        stdout,
        t(" CONNECTION TEST ", " 连接测试 "),
        move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            runtime.block_on(async {
            if draft.kind == ModelEndpointKind::Jev {
                let connection = crate::config::jev_connection_for(Some(&draft))?;
                return Ok(crate::jev::probe::probe(&connection).await.summary());
            }
            let directory = tempfile::tempdir()?;
            let result = crate::tools::image_generation::generate(
                serde_json::json!({"endpoint_id": draft.id, "prompt": "A simple geometric test image", "resolution": "256x256"}),
                vec![draft], directory.path().to_path_buf(),
                crate::tools::ToolProgress::new(tokio::sync::mpsc::unbounded_channel().0),
            );
            tokio::time::timeout(std::time::Duration::from_secs(30), result).await??;
            Ok(t("Image endpoint returned a valid image", "生图接入返回了有效图片").into())
        })
        },
    )?;
    if let Some(result) = result {
        message(stdout, &result.unwrap_or_else(|error| error))?;
    }
    Ok(())
}
