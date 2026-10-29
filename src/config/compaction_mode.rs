//! 压缩策略的触发模式；缺省保留旧配置的双条件语义。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactionMode {
    #[default]
    Legacy,
    Ratio,
    Reserve,
}

impl CompactionMode {
    /// 【上下文】【策略兼容】省略旧模式字段，避免无关配置保存产生迁移。
    /// 参数：当前模式；返回：是否使用旧双条件计算。
    pub fn is_legacy(&self) -> bool {
        *self == Self::Legacy
    }
}

#[cfg(test)]
mod tests {
    use super::CompactionMode;
    use crate::config::ContextConfig;

    /// 【上下文】【配置兼容回归】旧配置保存不新增模式，明确模式保存后重载仍按所选条件计算。
    /// 参数：无；返回：无，兼容字段或模式序列化异常时断言失败。
    #[test]
    fn context_mode_round_trip_preserves_legacy_defaults() {
        let context: ContextConfig = serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(context.compaction_mode, CompactionMode::Legacy);
        assert!(serde_json::to_value(&context)
            .unwrap()
            .get("compaction_mode")
            .is_none());
        for mode in [CompactionMode::Ratio, CompactionMode::Reserve] {
            let context = ContextConfig {
                compaction_mode: mode,
                ..context.clone()
            };
            let saved = serde_json::to_value(&context).unwrap();
            let restored: ContextConfig = serde_json::from_value(saved).unwrap();
            assert_eq!(restored.compaction_mode, mode);
        }
    }
}
