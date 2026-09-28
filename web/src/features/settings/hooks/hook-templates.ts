import type { HookItem } from "../../../api/contracts";

/** 生命周期事件清单，与后端事件名称一致。 */
export const HOOK_EVENTS = [
  ["agent_start", "Agent start", "Agent 开始"], ["agent_end", "Agent end", "Agent 结束"],
  ["turn_start", "Turn start", "轮次开始"], ["turn_end", "Turn end", "轮次结束"],
  ["message_start", "Message start", "消息开始"], ["message_end", "Message end", "消息结束"],
  ["tool_execution_start", "Tool start", "工具开始"], ["tool_execution_end", "Tool end", "工具结束"]
] as const;

/** 可编辑的诊断模板；钩子失败不会阻断主流程。 */
export const HOOK_TEMPLATES = [
  { id: "event-log", en: "Event log", zh: "事件记录", descriptionEn: "Record the event and session in command output.", descriptionZh: "在命令输出中记录事件与会话。", patch: { event: "agent_end", script: 'printf \'%s\\n\' "$SAI_HOOK_EVENT session=$SAI_SESSION_ID"' } },
  { id: "diff-check", en: "Check whitespace", zh: "检查差异空白", descriptionEn: "Run git diff --check after each turn; reports only, does not block commits.", descriptionZh: "轮次结束后执行 git diff --check；仅报告，不阻止提交。", patch: { event: "turn_end", script: 'cd "$SAI_WORKDIR" && git diff --check' } }
] as const;

/**
 * 【Hooks】【创建】生成唯一名称并复制模板，保持新增配置可独立编辑。
 * @param items 当前钩子列表
 * @param patch 模板字段
 * @returns 新钩子配置
 */
export function createHook(items: HookItem[], patch: Partial<HookItem> = {}): HookItem {
  let index = items.length + 1;
  while (items.some((item) => item.name === `hook-${index}`)) index += 1;
  return { enabled: true, event: "agent_end", kind: "command", script: HOOK_TEMPLATES[0].patch.script, timeout_ms: 30_000, requests: [], ...patch, name: `hook-${index}` };
}

/**
 * 生成允许重复名称的列表标识。
 * @param item 钩子配置
 * @param index 列表位置
 * @returns 地址中的对象标识
 */
export function hookItemKey(item: HookItem, index: number): string {
  return `${index}:${item.name}`;
}
