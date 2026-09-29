//! 【Jev路由】【上下文注入】收集每轮候选并渲染命中正文，索引仍交由原有投影去重。
use super::Agent;
use crate::jev::{prompt_segments, Candidate, CandidateKind, Selection};
use anyhow::Result;

const MEMORY_ID: &str = "memory_context";

/// 本轮提示词候选与正文快照，保证判断和注入使用同一份内容。
#[derive(Default)]
pub(super) struct PromptContext {
    pub candidates: Vec<Candidate>,
    fragments: Vec<(String, String)>,
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
                context
                    .fragments
                    .push((MEMORY_ID.to_string(), contract.to_string()));
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
            if self.fragments.iter().any(|(known, _)| known == &id) {
                continue;
            }
            self.candidates.push(Candidate::new(
                CandidateKind::Prompt,
                &id,
                &segment.description,
            ));
            self.fragments.push((id, segment.content));
        }
        Ok(())
    }

    /// 【Jev路由】【正文注入】参数为本轮选择；返回按来源顺序排列的命中正文及记忆开关。
    pub(super) fn render(&self, selection: &Selection) -> Preselection {
        let mut blocks = Vec::new();
        for (id, content) in &self.fragments {
            if selection.prompts.contains(id) && !content.is_empty() {
                blocks.push(format!(
                    "<jev-context id=\"{id}\">\n{content}\n</jev-context>"
                ));
            }
        }
        Preselection {
            block: (!blocks.is_empty()).then(|| format!(
                "<jev-selected-context>\nThe Jev router selected these configured instructions for the current request. Apply them when relevant to this request.\n{}\n</jev-selected-context>",
                blocks.join("\n\n")
            )),
            memory_selected: selection.prompts.iter().any(|id| id == MEMORY_ID),
        }
    }
}
