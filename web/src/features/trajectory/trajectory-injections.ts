import { parseJevExposureBlock } from "../chat/tool-renderers/jev-capability-data";
import { summarizeContent, summarizeJevExposure } from "./trajectory-format";

/**
 * 将发送前的 Jev 决策与其余动态上下文拆成独立轨迹记录。
 * @param content 当前轮次的完整注入前缀
 * @returns 保持内容来源独立的注入片段
 */
export function trajectoryInjections(content: string) {
  const exposure = parseJevExposureBlock(content);
  // 工具/Skill 暴露块与片段/记忆注入块都归入 Jev 记录，其余动态上下文单独列出
  const blocks = [
    /<jev-exposed-capabilities>[\s\S]*?<\/jev-exposed-capabilities>/u.exec(content)?.[0],
    /<jev-selected-context>[\s\S]*?<\/jev-selected-context>/u.exec(content)?.[0]
  ].filter((block): block is string => Boolean(block));
  const context = blocks.reduce((rest, block) => rest.replace(block, ""), content).trim();
  const entries = [];
  if (exposure && blocks.length > 0) {
    const jevContent = blocks.join("\n\n");
    entries.push({ id: "jev", label: "Jev", content: jevContent, summary: summarizeJevExposure(jevContent) ?? "Jev", exposure });
  }
  if (context) entries.push({ id: "injected", label: injectedLabel(context), content: context, summary: summarizeContent(context), exposure: undefined });
  return entries;
}

/** 根据注入标签给出来源；参数为注入正文，返回简短来源名称。 */
function injectedLabel(content: string): string {
  const tags: string[] = [];
  if (content.includes("<context-state")) tags.push("context-state");
  if (content.includes("instruction_files") || content.includes("<instruction-files")) tags.push("AGENT.md");
  if (content.includes("<context-resource")) tags.push("resource");
  if (content.includes("<memory")) tags.push("memory");
  if (content.includes("<mode-instructions")) tags.push("mode");
  return tags.join(" · ") || "inject";
}
