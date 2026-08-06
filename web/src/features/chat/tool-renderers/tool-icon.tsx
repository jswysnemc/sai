import {
  Check,
  CircleDashed,
  FileCode2,
  FilePenLine,
  FileSearch,
  FolderTree,
  Globe,
  ListTodo,
  Search,
  ShieldAlert,
  Sparkles,
  TerminalSquare,
  Trash2,
  Wrench,
  X
} from "lucide-react";
import type { ToolCardTone } from "./tool-card-shell";

/**
 * 按工具语义返回图标。
 *
 * 图标表达"这是什么操作"，与状态无关；状态由色调与状态标记表达。
 *
 * @param name 工具标识
 * @returns 对应的图标元素
 */
export function ToolIcon({ name }: { name: string }) {
  const size = 14;
  if (name === "run_command" || name.includes("command")) return <TerminalSquare size={size} />;
  if (name === "edit_file" || name === "write_file" || name === "str_replace") return <FilePenLine size={size} />;
  if (name === "read_file") return <FileSearch size={size} />;
  if (name === "grep") return <Search size={size} />;
  if (name === "glob" || name === "list_dir") return <FolderTree size={size} />;
  if (name === "todo") return <ListTodo size={size} />;
  if (name === "trash_path" || name.includes("delete")) return <Trash2 size={size} />;
  if (name.includes("web") || name.includes("search")) return <Globe size={size} />;
  if (name.includes("skill") || name.includes("agent")) return <Sparkles size={size} />;
  if (name === "load" || name.includes("file")) return <FileCode2 size={size} />;
  return <Wrench size={size} />;
}

/** 卡片可呈现的状态 */
export type ToolCardState = "preparing" | "running" | "completed" | "failed" | "pending" | "allowed" | "denied";

/**
 * 按状态返回标记图标。
 *
 * 只在有明确结论时给出标记：进行中的卡片靠图标呼吸表达，不再多画一个转圈。
 *
 * @param state 卡片状态
 * @returns 状态标记元素，无需标记时返回 null
 */
export function ToolStatusMark({ state }: { state: ToolCardState }) {
  if (state === "completed" || state === "allowed") return <Check size={13} />;
  if (state === "failed" || state === "denied") return <X size={13} />;
  if (state === "pending") return <ShieldAlert size={13} />;
  if (state === "preparing") return <CircleDashed className="tool-arguments-spinner" size={13} />;
  return null;
}

/**
 * 把卡片状态映射到外壳色调。
 *
 * @param state 卡片状态
 * @returns 外壳色调
 */
export function toneOfState(state: ToolCardState): ToolCardTone {
  switch (state) {
    case "completed":
    case "allowed":
      return "success";
    case "failed":
    case "denied":
      return "danger";
    case "pending":
      return "attention";
    case "running":
      return "running";
    default:
      return "neutral";
  }
}
