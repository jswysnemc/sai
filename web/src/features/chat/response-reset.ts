import type { LiveRunState } from "./run-event-reducer";

/** 【模型恢复】【文本撤销】参数为文本和 Unicode 字符数；返回移除末尾片段后的文本 */
function trimSuffix(text: string, count: number): string {
  const characters = Array.from(text);
  return characters.slice(0, Math.max(0, characters.length - count)).join("");
}

/** 【模型恢复】【响应撤销】参数为当前状态、失败片段字符数及压缩标识；返回保留已完成内容的状态 */
export function resetResponse(state: LiveRunState, content: number, reasoning: number, compaction: boolean): LiveRunState {
  let remainingContent = content;
  let remainingReasoning = reasoning;
  const parts = [...state.parts];
  for (let index = parts.length - 1; index >= 0; index -= 1) {
    const part = parts[index];
    if (compaction) {
      if (part.type === "compaction" && part.status === "running") {
        parts[index] = { ...part, summary: trimSuffix(part.summary ?? "", content) };
        break;
      }
      continue;
    }
    if (part.type === "tool" && part.tool.status === "preparing") {
      parts.splice(index, 1);
    } else if (part.type === "text" && remainingContent > 0) {
      const source = trimSuffix(part.source, remainingContent);
      remainingContent = Math.max(0, remainingContent - Array.from(part.source).length);
      if (source) parts[index] = { ...part, source };
      else parts.splice(index, 1);
    } else if (part.type === "reasoning" && remainingReasoning > 0) {
      const source = trimSuffix(part.source, remainingReasoning);
      remainingReasoning = Math.max(0, remainingReasoning - Array.from(part.source).length);
      if (source) parts[index] = { ...part, source };
      else parts.splice(index, 1);
    }
  }
  return {
    ...state,
    parts,
    content: compaction ? state.content : trimSuffix(state.content, content),
    reasoning: compaction ? state.reasoning : trimSuffix(state.reasoning, reasoning),
    tools: compaction ? state.tools : state.tools.filter((tool) => tool.status !== "preparing")
  };
}
