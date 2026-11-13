pub(crate) mod template;

#[cfg(test)]
mod template_tests;

pub const YOLO_REMINDER: &str = include_str!("prompts/yolo.md");
pub const AUDITED_REMINDER: &str = include_str!("prompts/audited.md");
pub const AUTO_AUDIT_REMINDER: &str = include_str!("prompts/auto-audit-mode.md");
pub const AUTO_AUDIT_SYSTEM_PROMPT: &str = include_str!("prompts/auto-audit.md");
pub const PLAN_REMINDER: &str = include_str!("prompts/plan.md");
