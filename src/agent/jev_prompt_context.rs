//! 【Jev路由】【上下文注入】收集每轮候选并渲染命中正文，索引仍交由原有投影去重。
use super::Agent;
use crate::jev::{prompt_segments, Candidate, CandidateKind, Selection};
use anyhow::Result;

const MEMORY_ID: &str = "memory_context";

/// 注入片段的种类，界面据此区分提示词片段与记忆。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FragmentKind {
    /// 系统提示或指令文件里的 `<jev>` 片段
    Prompt,
    /// 记忆索引与使用契约
    Memory,
}

impl FragmentKind {
    /// 注入标签与界面使用的种类标识。
    ///
    /// 返回:
    /// - prompt 或 memory
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Prompt => "prompt",
            Self::Memory => "memory",
        }
    }
}

/// 一个可注入片段及其来源信息。
pub(super) struct Fragment {
    /// 候选标识
    pub id: String,
    pub kind: FragmentKind,
    /// 来源：system、instructions、extra 或 memory
    pub source: String,
    /// 界面展示用的简短说明
    pub description: String,
    /// 命中后注入的正文
    pub content: String,
    /// 记忆候选附带的索引，只用于界面预览
    pub preview: Option<String>,
}

/// 本轮提示词候选与正文快照，保证判断和注入使用同一份内容。
#[derive(Default)]
pub(super) struct PromptContext {
    pub candidates: Vec<Candidate>,
    fragments: Vec<Fragment>,
}

/// 请求前路由结果，明确区分记忆命中与能力暴露文本。
#[derive(Default)]
pub(super) struct Preselection {
    pub block: Option<String>,
    pub memory_selected: bool,
}

impl Agent {
    /// 【Jev路由】【候选收集】参数为本轮可用索引；返回配置、指令与记忆候选快照。
    pub(super) fn jev_prompt_context(&self, memory_index: Option<&str>) -> Result<PromptContext> {
        let mut context = PromptContext::default();
        // 1. 标签片段开关关闭时片段原文留在静态提示里，这里不再生成候选
        if self.config.jev_prompt_segments_active() {
            context.add_source("system", &self.config.system_prompt(&self.paths)?)?;
            if self.config.load_instruction_files {
                context.add_source(
                    "instructions",
                    &super::instruction_files::load_instruction_prompt(&self.paths),
                )?;
            }
            if let Some(extra) = self.extra_system_prompt.as_deref() {
                context.add_source("extra", extra)?;
            }
        }
        // 2. 记忆索引和契约共用一次判断，避免只提供索引而缺少使用说明
        if self.config.jev_memory_injection_active() && self.config.prompt_sections.memory_contract
        {
            let contract = if self.tools_enabled
                && super::system_prompt::memory_tools_reach_model(&self.config)
            {
                crate::memory::file_store::memory_contract()
            } else {
                ""
            };
            if memory_index.is_some() || !contract.is_empty() {
                let description = format!(
                    "Recall stored user preferences, project facts or previous decisions, or save durable information explicitly requested by the user. Memory index and read/write guidance. Available index: {}",
                    memory_index.unwrap_or("No saved memories yet; consider whether this request requires saving a durable preference or decision.")
                );
                context.candidates.push(Candidate::new(
                    CandidateKind::Prompt,
                    MEMORY_ID,
                    &description,
                ));
                context.fragments.push(Fragment {
                    id: MEMORY_ID.to_string(),
                    kind: FragmentKind::Memory,
                    source: "memory".to_string(),
                    description: memory_label(memory_index).to_string(),
                    content: contract.to_string(),
                    preview: memory_index.map(str::to_string),
                });
            }
        }
        Ok(context)
    }
}

impl PromptContext {
    /// 解析单个来源；参数为来源标识和原文，返回解析结果并记录稳定候选标识。
    fn add_source(&mut self, source: &str, text: &str) -> Result<()> {
        for segment in prompt_segments::parse(text)?.segments {
            let hash = blake3::hash(
                format!("{source}\n{}\n{}", segment.description, segment.content).as_bytes(),
            );
            let id = format!("prompt_{}", &hash.to_hex()[..24]);
            if self.fragments.iter().any(|fragment| fragment.id == id) {
                continue;
            }
            self.candidates.push(Candidate::new(
                CandidateKind::Prompt,
                &id,
                &segment.description,
            ));
            self.fragments.push(Fragment {
                id,
                kind: FragmentKind::Prompt,
                source: source.to_string(),
                description: segment.description,
                content: segment.content,
                preview: None,
            });
        }
        Ok(())
    }

    /// 【Jev路由】【正文注入】参数为本轮选择和近期历史；已在上下文中的片段只给提示。
    ///
    /// 参数:
    /// - `selection`: Jev 选择结果
    /// - `history`: 近期消息序列化文本，用于判断片段是否已注入
    ///
    /// 返回:
    /// - 命中正文或已存在提示，以及记忆开关
    pub(super) fn render(&self, selection: &Selection, history: &str) -> Preselection {
        let mut blocks = Vec::new();
        let mut already = Vec::new();
        for fragment in self.selected(selection) {
            if fragment.content.is_empty() {
                continue;
            }
            let marker = format!("<jev-context id=\"{}\"", fragment.id);
            if history.contains(&marker) || history.contains(&fragment.content) {
                already.push(fragment.description.clone());
                continue;
            }
            // 种类、来源与说明写进标签，历史回放时界面无需再查候选表
            blocks.push(format!(
                "<jev-context id=\"{}\" kind=\"{}\" source=\"{}\" description=\"{}\">\n{}\n</jev-context>",
                fragment.id,
                fragment.kind.as_str(),
                fragment.source,
                escape_attribute(&fragment.description),
                fragment.content
            ));
        }
        let mut parts = Vec::new();
        if !already.is_empty() {
            parts.push(format!(
                "<jev-context-hint>\nThese configured instructions are already in the current context; do not re-inject them: {}.\n</jev-context-hint>",
                already.join("; ")
            ));
        }
        if !blocks.is_empty() {
            parts.push(format!(
                "<jev-selected-context>\nThe Jev router selected these configured instructions for the current request. Apply them when relevant to this request.\n{}\n</jev-selected-context>",
                blocks.join("\n\n")
            ));
        }
        Preselection {
            block: (!parts.is_empty()).then(|| parts.join("\n\n")),
            memory_selected: selection.prompts.iter().any(|id| id == MEMORY_ID),
        }
    }

    /// 本轮命中的片段，按来源顺序排列。
    ///
    /// 参数:
    /// - `selection`: Jev 选择结果
    ///
    /// 返回:
    /// - 命中片段
    pub(super) fn selected<'a>(
        &'a self,
        selection: &'a Selection,
    ) -> impl Iterator<Item = &'a Fragment> + 'a {
        self.fragments
            .iter()
            .filter(move |fragment| selection.prompts.contains(&fragment.id))
    }
}

/// 记忆候选的界面说明：有索引时说明可用记忆，没有时说明只提供使用契约。
///
/// 参数:
/// - `memory_index`: 本轮可用记忆索引
///
/// 返回:
/// - 简短说明
fn memory_label(memory_index: Option<&str>) -> &'static str {
    if memory_index.is_some() {
        "Memory index and read/write guidance"
    } else {
        "Memory read/write guidance"
    }
}

/// 转义标签属性值中的引号、尖括号与换行。
///
/// 参数:
/// - `value`: 属性原文
///
/// 返回:
/// - 可放进双引号属性的单行文本
fn escape_attribute(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
