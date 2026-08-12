pub(crate) const RESET: &str = "\x1b[0m";
pub(crate) const PRIMARY_STYLE: &str = "\x1b[38;5;189m";
pub(crate) const SECONDARY_STYLE: &str = "\x1b[36m";
// 以下旧 Markdown 样式已由文件末尾的 MD_* 体系取代；
// 值保持不变仅为避免与并行改动冲突，请勿在新代码中引用。
#[allow(dead_code)]
pub(crate) const TERTIARY_STYLE: &str = "\x1b[35m";
#[allow(dead_code)]
pub(crate) const HEADER_STYLE: &str = "\x1b[1m\x1b[35m";
#[allow(dead_code)]
pub(crate) const INLINE_CODE_STYLE: &str = SECONDARY_STYLE;
#[allow(dead_code)]
pub(crate) const LINK_LABEL_STYLE: &str = "\x1b[4m\x1b[38;5;81m";
#[allow(dead_code)]
pub(crate) const URL_STYLE: &str = "\x1b[2m\x1b[38;5;75m";
#[allow(dead_code)]
pub(crate) const IMAGE_STYLE: &str = "\x1b[1m\x1b[38;5;213m";
/// 脚注引用标记（正文中的 `[^1]`）。
pub(crate) const FOOTNOTE_REF_STYLE: &str = "\x1b[38;5;110m";
/// 脚注定义行的序号标记（`[^1]:` 行首）。
pub(crate) const FOOTNOTE_DEF_STYLE: &str = "\x1b[1m\x1b[38;5;110m";
#[allow(dead_code)]
pub(crate) const BOLD_STYLE: &str = "\x1b[1m\x1b[34m";
#[allow(dead_code)]
pub(crate) const ITALIC_STYLE: &str = "\x1b[3m\x1b[38;5;250m";
#[allow(dead_code)]
pub(crate) const STRIKE_STYLE: &str = "\x1b[9m";
#[allow(dead_code)]
pub(crate) const CODE_BLOCK_FRAME_STYLE: &str = SECONDARY_STYLE;
pub(crate) const CODE_TOKEN_RESET: &str = "\x1b[0m";
pub(crate) const CODE_KEYWORD_STYLE: &str = "\x1b[38;2;196;167;231m";
pub(crate) const CODE_FUNCTION_STYLE: &str = "\x1b[38;2;156;207;216m";
pub(crate) const CODE_STRING_STYLE: &str = "\x1b[38;2;166;214;160m";
pub(crate) const CODE_NUMBER_STYLE: &str = "\x1b[38;2;246;193;119m";
pub(crate) const CODE_COMMENT_STYLE: &str = "\x1b[2m\x1b[38;2;110;106;134m";
pub(crate) const TABLE_BORDER_STYLE: &str = "\x1b[2m";
pub(crate) const ASSET_ERROR_STYLE: &str = "\x1b[31m";
pub(crate) const TOOL_BULLET: &str = "•";

// ── Markdown 正文统一视觉体系 ────────────────────────────────────
//
// 设计原则（与转录区引导符号 `•`/`◦`/`›`、命令输出 dim gutter 同一语言）：
// 1. 结构符号一律弱化（dim）：列表符号、引用竖线、分隔线、语言标签、
//    表格边框、URL——让内容凸显、骨架后退；
// 2. 层级靠字重：标题不占用色相，H1 加下划线、H2 加粗、H3+ 加粗弱化；
// 3. 少量语义点缀色：行内代码浅驼（180）、链接蓝（75）、脚注蓝灰（110），
//    正文中不再出现其余色相，避免与工具区 cyan、diff 红绿抢占注意力。

/// 一级标题：加粗 + 下划线（全文唯一使用下划线的块级元素）。
pub(crate) const MD_H1_STYLE: &str = "\x1b[1m\x1b[4m";
/// 二级标题：加粗。
pub(crate) const MD_H2_STYLE: &str = "\x1b[1m";
/// 三级及以下标题：加粗 + 弱化，比正文醒目、低于二级。
pub(crate) const MD_H3_STYLE: &str = "\x1b[1m\x1b[2m";
/// 列表符号（`-` 与有序数字）：弱化，与其他结构符号同灰阶。
pub(crate) const MD_LIST_MARKER_STYLE: &str = "\x1b[2m";
/// 引用块左侧竖线：弱化灰，不再借用绿色语义。
pub(crate) const MD_QUOTE_BAR_STYLE: &str = "\x1b[2m";
/// 行内代码：浅驼色点缀（256 色 180），无背景无加粗。
pub(crate) const MD_INLINE_CODE_STYLE: &str = "\x1b[38;5;180m";
/// 加粗强调：纯字重，不占用色相。
pub(crate) const MD_BOLD_STYLE: &str = "\x1b[1m";
/// 斜体强调：纯斜体。
pub(crate) const MD_ITALIC_STYLE: &str = "\x1b[3m";
/// 删除线：删除语义叠加弱化。
pub(crate) const MD_STRIKE_STYLE: &str = "\x1b[9m\x1b[2m";
/// 链接标签：下划线 + 链接蓝（256 色 75）。
pub(crate) const MD_LINK_LABEL_STYLE: &str = "\x1b[4m\x1b[38;5;75m";
/// 链接地址（含包裹括号）：弱化的链接蓝。
pub(crate) const MD_URL_STYLE: &str = "\x1b[2m\x1b[38;5;75m";
/// 图片占位符：弱化的链接蓝，与链接同属"外部资源"族。
pub(crate) const MD_IMAGE_STYLE: &str = "\x1b[2m\x1b[38;5;75m";
/// 代码块语言标签：弱化，与命令输出块的 dim gutter 风格一致。
pub(crate) const MD_CODE_LANG_STYLE: &str = "\x1b[2m";
