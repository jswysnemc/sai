import { Ban, CheckCircle2, Circle, CircleDot } from "lucide-react";
import type { TodoStatus } from "../../../api/contracts";
import { parseTodoTool } from "./todo-tool-data";
import "./todo-tool-view.css";

const statusIcons = { pending: Circle, in_progress: CircleDot, completed: CheckCircle2, cancelled: Ban } satisfies Record<TodoStatus, typeof Circle>;

type TodoToolItem = { id: string; text: string; status: TodoStatus };

/**
 * 渲染 todo 工具展开后的清单，头部由统一工具行承担。
 *
 * @param props todo 工具调用的参数与输出
 * @returns 清单列表；没有条目时为空
 */
export function TodoToolView({ argumentsText, output }: { toolId?: string; argumentsText: string; output: string }) {
  const summary = parseTodoTool(argumentsText, output);
  const items = parseItems(output);
  const changed = new Set(summary.changedIds);
  if (items.length === 0) return null;
  return (
    <ul className="todo-tool-list">
      {items.map((item) => {
        const Icon = statusIcons[item.status] ?? Circle;
        return (
          <li key={item.id} className={`todo-tool-item is-${item.status}${changed.has(item.id) ? " is-changed" : ""}`}>
            <Icon size={14} /><span>{item.text}</span>
          </li>
        );
      })}
    </ul>
  );
}

/**
 * 从 todo 工具输出中解析清单条目。
 *
 * 优先取 items 全量快照;旧格式输出没有 items 时回退本次变更条目,
 * 保证创建/更新/删除卡片也有可展开的内容。
 *
 * @param output todo 工具输出 JSON
 * @returns 清单条目,无法解析时为空数组
 */
function parseItems(output: string): TodoToolItem[] {
  try {
    const value = JSON.parse(output) as { items?: unknown; changed?: unknown; item?: unknown };
    if (Array.isArray(value.items)) return value.items.filter(isTodoItem);
    if (Array.isArray(value.changed)) return value.changed.filter(isTodoItem);
    if (isTodoItem(value.item)) return [value.item];
    return [];
  } catch {
    return [];
  }
}

/** 判断值是否为合法的 todo 条目。 */
function isTodoItem(value: unknown): value is TodoToolItem {
  return typeof value === "object" && value !== null
    && typeof (value as TodoToolItem).id === "string"
    && typeof (value as TodoToolItem).text === "string"
    && typeof (value as TodoToolItem).status === "string";
}
