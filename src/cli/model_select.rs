use super::models_picker::{self, PickerOutcome};
use super::*;

/// 交互式选择模型（与 `sai models` 共用双列选择器）。
///
/// 保存成功时返回 true；取消时返回 false。思考等级也会一并写回配置。
///
/// 参数:
/// - `paths`: Sai 路径
///
/// 返回:
/// - Ok(true) 已保存；Ok(false) 取消；错误时 Err
#[allow(dead_code)]
pub(super) fn select_model_interactively(paths: &SaiPaths) -> Result<bool> {
    match models_picker::run_interactive(paths)? {
        PickerOutcome::Cancelled => Ok(false),
        PickerOutcome::Saved { .. } => Ok(true),
    }
}
