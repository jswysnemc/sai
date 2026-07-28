import { ShieldCheck } from "lucide-react";
import { useQuery } from "@tanstack/react-query";
import { api } from "../../api/client";
import { usePersistedExpand } from "./message/tool-expand-state";
import type { ToolLifecycle } from "./run-event-reducer";
import { toolCardSummary } from "./tool-renderers/tool-card-summary";
import { parseCodexSubagentActivity } from "./tool-renderers/codex-subagent-data";
import { CodexSubagentToolView } from "./tool-renderers/codex-subagent-tool-view";
import { toolFilePath } from "./tool-renderers/tool-data";
import { ToolFileReference } from "./tool-renderers/tool-file-reference";
import { displayPath } from "./tool-renderers/tool-display-summary";
import { ToolCardShell } from "./tool-renderers/tool-card-shell";
import { ToolIcon, ToolStatusMark, toneOfState } from "./tool-renderers/tool-icon";
import { ToolResultView } from "./tool-renderers/tool-result-view";
import { TodoToolView } from "./tool-renderers/todo-tool-view";
import "./tool-renderers/tool-renderers.css";
import { useI18n } from "../i18n/use-i18n";

/**
 * 渲染一项实时或历史工具生命周期。
 *
 * @param props 工具生命周期状态
 * @returns 统一外壳的可折叠工具卡片
 */
export function ToolLifecycleCard({ tool }: { tool: ToolLifecycle }) {
  const { locale, t } = useI18n();
  const workspaces = useQuery({ queryKey: ["workspaces"], queryFn: api.workspaces.list, staleTime: 30_000 });
  const workspacePath = workspaces.data?.workspaces.find((item) => item.id === workspaces.data?.active_id)?.path ?? "";
  // 失败默认展开；用户展开后按 tool.id 记忆，流式更新不自动收缩
  const [expanded, setExpanded] = usePersistedExpand(tool.id, tool.status === "failed");
  // 1. todo 已完成时改用清单卡片，不暴露原始 JSON
  if (tool.name === "todo" && tool.status === "completed") {
    return <TodoToolView toolId={tool.id} argumentsText={tool.arguments || tool.argumentsPreview} output={tool.output} />;
  }
  const argumentsText = tool.arguments || tool.argumentsPreview;
  const subagentActivity = parseCodexSubagentActivity(argumentsText);
  // 2. Codex 原生子智能体事件使用语义视图，不把协议参数作为唯一内容
  if (subagentActivity) {
    return (
      <CodexSubagentToolView
        tool={tool}
        activity={subagentActivity}
        expanded={expanded}
        onToggle={() => setExpanded((value) => !value)}
      />
    );
  }
  const headerPath = toolFilePath(tool.name, argumentsText);
  // 3. 操作对象优先取工作区相对路径，其次是参数摘要；准备阶段没有对象时留空
  const relativePath = headerPath ? displayPath(headerPath, workspacePath) : "";
  const summary = headerPath ? "" : toolCardSummary(tool.name, argumentsText, locale, workspacePath) || tool.progress;
  const target = headerPath
    ? <ToolFileReference path={headerPath} label={relativePath || headerPath} className="tool-shell-file" icon={false} />
    : summary;

  // 4. 权限已并入本卡：头部只留一枚徽章，理由放进展开区，不再单独占一张卡
  const permission = tool.permission;
  const autoAudited = permission?.decision === "allow" && permission.source === "auto_audit";
  const auditReason = permission?.decision === "allow" ? permission.reason?.trim() ?? "" : "";

  return (
    <ToolCardShell
      tone={toneOfState(tool.status)}
      icon={<ToolIcon name={tool.name} />}
      title={readableToolName(tool.name)}
      target={target || undefined}
      targetTitle={headerPath || summary || undefined}
      meta={
        tool.status === "preparing"
          ? t("Preparing", "准备中")
          : permission
            ? <ToolPermissionBadge autoAudited={autoAudited} t={t} />
            : undefined
      }
      status={tool.status === "completed" ? undefined : <ToolStatusMark state={tool.status} />}
      expanded={expanded}
      onToggle={() => setExpanded((value) => !value)}
    >
      {auditReason && (
        <div className="tool-permission-reason">
          <span>{t("Auto-audit reason", "自动审核理由")}</span>
          {auditReason}
        </div>
      )}
      <ToolResultView name={tool.name} argumentsText={argumentsText} output={tool.output} headerPath={headerPath} />
    </ToolCardShell>
  );
}

/**
 * 渲染工具卡头部的权限徽章。
 *
 * 只表达"这次调用是被批准的"以及批准来源，具体理由留给展开区，
 * 避免折叠态的一行里塞进整句说明。
 *
 * @param props autoAudited 表示由审核模型放行，t 为双语文本选择方法
 * @returns 权限徽章元素
 */
function ToolPermissionBadge({ autoAudited, t }: { autoAudited: boolean; t: (en: string, zh: string) => string }) {
  return (
    <span className={autoAudited ? "tool-permission-badge auto" : "tool-permission-badge"}>
      <ShieldCheck size={11} aria-hidden />
      {autoAudited ? t("Auto-approved", "自动放行") : t("Approved", "已批准")}
    </span>
  );
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
    background_command: "Shell",
    edit_file: "Edit",
    write_file: "Write",
    str_replace: "Replace",
    read_file: "Read",
    grep: "Search",
    glob: "Files",
    list_dir: "List",
    trash_path: "Trash",
    todo: "Todo",
    load: "Load"
  };
  return labels[name] ?? name.replaceAll("_", " ");
}
