import { Check, ChevronDown, CircleEllipsis, FilePenLine, FileSearch, Search, TerminalSquare, Wrench, X } from "lucide-react";
import { type KeyboardEvent } from "react";
import { usePersistedExpand } from "./message/tool-expand-state";
import type { ToolLifecycle } from "./run-event-reducer";
import { toolCardSummary } from "./tool-renderers/tool-card-summary";
import { toolFilePath } from "./tool-renderers/tool-data";
import { ToolFileReference } from "./tool-renderers/tool-file-reference";
import { ToolResultView } from "./tool-renderers/tool-result-view";
import { TodoToolView } from "./tool-renderers/todo-tool-view";
import "./tool-renderers/tool-renderers.css";
import { useI18n } from "../i18n/use-i18n";

/**
 * 渲染一项实时或历史工具生命周期。
 *
 * @param props 工具生命周期状态
 * @returns 可折叠工具卡片
 */
export function ToolLifecycleCard({ tool }: { tool: ToolLifecycle }) {
  const { locale, t } = useI18n();
  // 失败默认展开；用户展开后按 tool.id 记忆，流式更新不自动收缩
  const [expanded, setExpanded] = usePersistedExpand(tool.id, tool.status === "failed");
  // 1. todo 工具已完成时改用专门的清单卡片,不暴露原始 JSON
  if (tool.name === "todo" && tool.status === "completed") {
    return <TodoToolView toolId={tool.id} argumentsText={tool.arguments || tool.argumentsPreview} output={tool.output} />;
  }
  const statusIcon = tool.status === "completed"
    ? <Check size={14} />
    : tool.status === "failed"
      ? <X size={14} />
      : <CircleEllipsis size={14} className="pulse" />;
  const argumentsText = tool.arguments || tool.argumentsPreview;
  const headerPath = toolFilePath(tool.name, argumentsText);
  const displayName = readableToolName(tool.name);
  const summary = uniqueSummary(
    toolCardSummary(tool.name, argumentsText, locale) || tool.progress || statusLabel(tool.status, t),
    displayName
  );
  /**
   * 切换当前工具详情的展开状态。
   *
   * @returns 无返回值
   */
  const toggleExpanded = () => setExpanded((value) => !value);

  /**
   * 使用键盘操作头部空白区域时切换详情。
   *
   * @param event 工具卡头部键盘事件
   * @returns 无返回值
   */
  const handleHeaderKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.target !== event.currentTarget || (event.key !== "Enter" && event.key !== " ")) return;
    event.preventDefault();
    toggleExpanded();
  };
  return (
    <section className={`tool-card tool-inline-row ${tool.status}`}>
      <div className="tool-card-head" role="button" tabIndex={0} onClick={toggleExpanded} onKeyDown={handleHeaderKeyDown} aria-expanded={expanded}>
        <span className="tool-icon"><ToolIcon name={tool.name} /></span>
        <span className="tool-card-copy">
          <strong className="tool-card-name">{displayName}</strong>
          <span className="tool-card-summary" title={headerPath || summary}>
            {headerPath ? <ToolFileReference path={headerPath} className="tool-card-file" icon={false} /> : summary}
          </span>
        </span>
        <span className="tool-card-status" aria-hidden>{statusIcon}</span>
        <ChevronDown size={14} className={`tool-card-expand${expanded ? " rotate" : ""}`} aria-hidden />
      </div>
      {expanded && <div className="tool-detail"><ToolResultView name={tool.name} argumentsText={argumentsText} output={tool.output} headerPath={headerPath} /></div>}
    </section>
  );
}

/**
 * 移除与工具标题相同的摘要，避免折叠态重复展示同一文本。
 *
 * @param summary 候选摘要
 * @param displayName 工具展示名称
 * @returns 去重后的摘要
 */
function uniqueSummary(summary: string, displayName: string): string {
  return summary.trim().toLocaleLowerCase() === displayName.trim().toLocaleLowerCase() ? "" : summary;
}

/**
 * 按工具语义返回图标。
 *
 * @param props 工具名称
 * @returns 工具图标
 */
function ToolIcon({ name }: { name: string }) {
  if (name === "run_command" || name.includes("command")) return <TerminalSquare size={15} />;
  if (name === "edit_file") return <FilePenLine size={15} />;
  if (name === "read_file") return <FileSearch size={15} />;
  if (name === "grep" || name === "glob") return <Search size={15} />;
  return <Wrench size={15} />;
}

/**
 * 将工具标识转换为可读名称。
 *
 * @param name 工具标识
 * @returns 可读名称
 */
function readableToolName(name: string): string {
  const labels: Record<string, string> = {
    run_command: "Shell",
    edit_file: "Edit",
    read_file: "Read",
    grep: "Search",
    glob: "Files",
    load: "Load"
  };
  return labels[name] ?? name.replaceAll("_", " ");
}

/**
 * 返回工具状态中文标签。
 *
 * @param status 工具状态
 * @returns 状态标签
 */
function statusLabel(status: ToolLifecycle["status"], t: (en: string, zh: string) => string): string {
  return {
    preparing: t("Preparing arguments", "准备参数"),
    running: t("Running", "正在执行"),
    completed: t("Completed", "执行完成"),
    failed: t("Failed", "执行失败")
  }[status];
}
