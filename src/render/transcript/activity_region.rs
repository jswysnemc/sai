use super::fullscreen_parts::ParagraphKey;

/// 【终端】【执行链范围】描述链内独立视口的源坐标，标题位于首行，末行坐标不包含该行。
#[derive(Clone, Debug)]
pub(crate) struct ActivityRegion {
    pub(crate) key: ParagraphKey,
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) scrollable: bool,
    pub(crate) active: bool,
    pub(crate) footer: Option<usize>,
}
