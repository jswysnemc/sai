export type TodoToolAction = "list" | "add" | "update" | "remove" | "unknown";
import { text, type Locale } from "../../i18n/locale";

export type TodoToolSummary = {
  action: TodoToolAction;
  text: string;
  texts: string[];
  status: string;
  itemCount: number | null;
  changedIds: string[];
};

/**
 * 解析 todo 工具调用的参数与输出,提炼一条简要摘要。
 *
 * 兼容两代输出格式:新格式的修改动作返回 changed 数组与 items 全量快照,
 * 旧格式只返回单条 item。
 *
 * @param argumentsText todo 工具调用参数 JSON
 * @param output todo 工具调用输出 JSON
 * @returns todo 调用摘要
 */
export function parseTodoTool(argumentsText: string, output: string): TodoToolSummary {
  const args = safeParse(argumentsText);
  const result = safeParse(output);
  const action = normalizeAction(typeof args.action === "string" ? args.action : "");
  const items = Array.isArray(result.items) ? result.items : null;
  const changed = readChanged(result);
  const texts = readTexts(args, changed);
  return {
    action,
    text: texts[0] ?? "",
    texts,
    status: readStatus(args, changed[0] ?? null),
    itemCount: items ? items.length : null,
    changedIds: changed.map((item) => (typeof item.id === "string" ? item.id : "")).filter(Boolean)
  };
}

/**
 * 生成 todo 调用的中文摘要句。
 *
 * @param summary todo 调用摘要
 * @returns 摘要文本
 */
export function todoToolHeadline(summary: TodoToolSummary, locale: Locale = "zh-CN"): string {
  switch (summary.action) {
    case "add":
      if (summary.texts.length > 1) return text(locale, `Created ${summary.texts.length} tasks`, `创建 ${summary.texts.length} 个任务`);
      return summary.text ? text(locale, `Created task: ${summary.text}`, `创建任务：${summary.text}`) : text(locale, "Created a task", "创建了一个任务");
    case "update":
      return summary.text
        ? text(locale, `Updated task: ${summary.text}`, `更新任务：${summary.text}`)
        : summary.status
          ? text(locale, `Updated task status to ${statusLabel(summary.status, locale)}`, `更新任务状态为${statusLabel(summary.status, locale)}`)
          : text(locale, "Updated a task", "更新了任务");
    case "remove":
      return summary.text ? text(locale, `Deleted task: ${summary.text}`, `删除任务：${summary.text}`) : text(locale, "Deleted a task", "删除了一个任务");
    case "list":
      return summary.itemCount !== null ? text(locale, `Viewed plan checklist (${summary.itemCount} items)`, `查看计划清单（${summary.itemCount} 项）`) : text(locale, "Viewed plan checklist", "查看计划清单");
    default:
      return text(locale, "Updated plan checklist", "更新计划清单");
  }
}

/**
 * 返回 todo 状态的中文名称。
 *
 * @param status 状态标识
 * @returns 状态名称
 */
export function statusLabel(status: string, locale: Locale = "zh-CN"): string {
  return ({
    pending: text(locale, "Pending", "待处理"),
    in_progress: text(locale, "In progress", "进行中"),
    completed: text(locale, "Completed", "已完成"),
    cancelled: text(locale, "Cancelled", "已取消")
  } as Record<string, string>)[status] ?? status;
}

/** 解析 JSON,失败时返回空对象。 */
function safeParse(text: string): Record<string, unknown> {
  try {
    const value = JSON.parse(text);
    return isRecord(value) ? value : {};
  } catch {
    return {};
  }
}

/** 判断值是否为普通对象。 */
function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** 归一化 action 字段。 */
function normalizeAction(action: string): TodoToolAction {
  return (["list", "add", "update", "remove"] as const).find((item) => item === action) ?? "unknown";
}

/** 从输出中读取本次变更条目,新格式取 changed 数组,旧格式回退单条 item。 */
function readChanged(result: Record<string, unknown>): Record<string, unknown>[] {
  if (Array.isArray(result.changed)) return result.changed.filter(isRecord);
  if (isRecord(result.item)) return [result.item];
  return [];
}

/** 收集本次调用涉及的任务文本,参数优先,其次取变更条目。 */
function readTexts(args: Record<string, unknown>, changed: Record<string, unknown>[]): string[] {
  if (Array.isArray(args.texts)) {
    const texts = args.texts.filter((value): value is string => typeof value === "string" && Boolean(value.trim())).map((value) => value.trim());
    if (texts.length > 0) return texts;
  }
  if (typeof args.text === "string" && args.text.trim()) return [args.text.trim()];
  return changed.map((item) => (typeof item.text === "string" ? item.text : "")).filter(Boolean);
}

/** 优先从参数、其次从变更条目读取任务状态。 */
function readStatus(args: Record<string, unknown>, item: Record<string, unknown> | null): string {
  if (typeof args.status === "string" && args.status.trim()) return args.status.trim();
  if (item && typeof item.status === "string") return item.status;
  return "";
}
