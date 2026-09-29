use crate::{
    config_tui::form::{run_form, Field},
    i18n::text as t,
    paths::SaiPaths,
    state::ResumeTarget,
};
use anyhow::Result;
use std::io;

/// 【会话恢复】【旧目录定位】使用统一表单补充旧索引未保存的工作区目录。
/// 参数: paths 为应用路径，target 为原目标；返回: 已校验目标或取消
pub(super) fn locate(paths: &SaiPaths, mut target: ResumeTarget) -> Result<Option<ResumeTarget>> {
    if target.workspace_path.is_some() {
        return Ok(Some(target));
    }
    let mut fields = vec![Field::new(
        t("Original workspace directory", "原工作区目录"),
        String::new(),
    )];
    loop {
        if !run_form(
            &mut io::stdout(),
            t(" LOCATE WORKSPACE ", " 定位原工作区 "),
            &mut fields,
        )? {
            return Ok(None);
        }
        target.workspace_path = Some(std::path::PathBuf::from(fields[0].value.trim()));
        match target.directory(paths) {
            Ok(path) => {
                target.workspace_path = Some(path);
                return Ok(Some(target));
            }
            Err(error) => super::super::center_panel::show_center_panel(
                t("Workspace mismatch", "工作区不匹配"),
                &error.to_string(),
            )?,
        }
    }
}
