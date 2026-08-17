import { ShieldCheck } from "lucide-react";
import { memo, useMemo, type ReactNode } from "react";
import { useQuery } from "@tanstack/react-query";
import { api } from "../../api/client";
import { usePersistedExpand } from "./message/tool-expand-state";
import type { ToolLifecycle } from "./run-event-reducer";
import { toolCardSummary } from "./tool-renderers/tool-card-summary";
import { parseCodexSubagentActivity } from "./tool-renderers/codex-subagent-data";
import { CodexSubagentToolView } from "./tool-renderers/codex-subagent-tool-view";
import { parseJsonRecord, stringField, toolFilePath } from "./tool-renderers/tool-data";
import { streamedDiffCounts } from "./tool-renderers/write-progress";
import { ToolCardActions } from "./tool-renderers/tool-card-actions";
import { ToolFileReference } from "./tool-renderers/tool-file-reference";
import { displayPath, isCommandToolName, isEditToolName } from "./tool-renderers/tool-display-summary";
import { toolDiffStat, toolResultSummary } from "./tool-renderers/tool-result-summary";
import { toolDurationLabel } from "./tool-renderers/tool-duration";
import { useElapsedClock } from "./tool-renderers/use-elapsed-clock";
import { HighlightedShellCommand } from "./tool-renderers/shell-command-line";
import { ToolLayout } from "./tool-renderers/layout/tool-layout";
import { ToolPanel } from "./tool-renderers/layout/tool-panel";
import { ToolIcon } from "./tool-renderers/tool-icon";
import { ToolResultView } from "./tool-renderers/tool-result-view";
import { TodoToolView } from "./tool-renderers/todo-tool-view";
import { parseTodoTool, todoToolHeadline } from "./tool-renderers/todo-tool-data";
import "./tool-renderers/tool-renderers.css";
import { useI18n } from "../i18n/use-i18n";

/**
 * 渲染一项实时或历史工具生命周期。
 *
 * @param props 工具生命周期状态
 * @returns 统一外壳的可折叠工具卡片
 */
export const ToolLifecycleCard = memo(function ToolLifecycleCard({
  tool,
  batchLabel
}: {
  tool: ToolLifecycle;
  batchLabel?: string;
}) {
  const { locale, t } = useI18n();
  const workspaces = useQuery({ queryKey: ["workspaces"], queryFn: api.workspaces.list, staleTime: 30_000 });
  const workspacePath = workspaces.data?.workspaces.find((item) => item.id === workspaces.data?.active_id)?.path ?? "";
  // 失败默认展开；用户展开后按 tool.id 记忆，流式更新不自动收缩
  const [expanded, setExpanded] = usePersistedExpand(tool.id, tool.status === "failed");
  // 执行中的卡片需要推进计时；结束后停表，历史卡片不占用任何定时器。
  // 编辑类不展示耗时（进度由行数与增删统计表达），也就不需要时钟
  const running = tool.status === "preparing" || tool.status === "running";
  const isEdit = isEditToolName(tool.name);
  const now = useElapsedClock(running && !isEdit);
  const argumentsText = tool.arguments || tool.argumentsPreview;
  const parsedArguments = useMemo(() => parseJsonRecord(argumentsText), [argumentsText]);
  const liveDiff = useMemo(
    () => (running && isEdit ? streamedDiffCounts(argumentsText) : null),
    [argumentsText, isEdit, running]
  );
  const todoSummary = tool.name === "todo" ? parseTodoTool(argumentsText, tool.output) : null;
  const todoHeadline = todoSummary ? todoToolHeadline(todoSummary, locale) : "";
  // 后台任务工具的管理操作（list/output/wait/stop/cleanup）不是 shell 命令，
  // 名称、图标与摘要都按动作语义表达
  const backgroundAction = tool.name.includes("background_command")
    ? stringField(parsedArguments, "action")
    : "";
  const backgroundManagement = Boolean(backgroundAction) && backgroundAction !== "start";
  const subagentActivity = parseCodexSubagentActivity(argumentsText);
  // Codex 原生子智能体事件使用语义视图，不把协议参数作为唯一内容
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
  // 操作对象：命令类展示完整命令占满剩余宽度，放不下才截断；
  //    其余优先取工作区相对路径，再退回参数摘要
  const isCommand = isCommandToolName(tool.name);
  const fullCommand = isCommand
    ? stringField(parsedArguments, "command")
      || stringField(parsedArguments, "cmd")
    : "";
  const relativePath = headerPath ? displayPath(headerPath, workspacePath) : "";
  const summary = todoHeadline
    || (headerPath ? "" : fullCommand || toolCardSummary(tool.name, argumentsText, locale, workspacePath) || tool.progress);
  const target = headerPath
    ? <ToolFileReference path={headerPath} label={relativePath || headerPath} className="tool-shell-file" icon={false} />
    : fullCommand
      ? <HighlightedShellCommand command={fullCommand} />
      : undefined;
  // 展开后详情区已有完整的 `$ 命令` 行，头部再放一遍就是冗余信息
  const hideTarget = expanded && isCommand && Boolean(fullCommand);

  // 权限已并入本卡：头部只留一枚徽章，理由放进展开区，不再单独占一张卡
  const permission = tool.permission;
  const autoAudited = permission?.decision === "allow" && permission.source === "auto_audit";
  const auditReason = permission?.decision === "allow" ? permission.reason?.trim() ?? "" : "";

  // 折叠行右段依次表达"结果如何"与"花了多久"；
  //    编辑类用 +N -M 徽章（流式参数阶段实时跳动，与 TUI 一致），不展示耗时
  const result = toolResultSummary(tool.name, tool.output, locale);
  const diffStat = toolDiffStat(tool.name, tool.output);
  const displayDiff = diffStat ?? liveDiff;
  const duration = isEdit ? "" : toolDurationLabel(tool.startedAtMs, tool.endedAtMs, now);
  const todoStatus = todoSummary?.itemCount != null
    ? t(`${todoSummary.itemCount} items`, `${todoSummary.itemCount} 项`)
    : undefined;

  return (
    <ToolLayout
      icon={<ToolIcon name={tool.name} backgroundTask={backgroundManagement} />}
      kindLabel={readableToolName(tool.name, backgroundManagement)}
      kindDetail={permission ? <ToolPermissionBadge autoAudited={autoAudited} t={t} /> : undefined}
      primaryContent={hideTarget ? undefined : target}
      primaryText={hideTarget || target ? "" : summary}
      secondaryText=""
      title={hideTarget ? undefined : fullCommand || headerPath || summary || undefined}
      diffCount={displayDiff ? { added: displayDiff.added, removed: displayDiff.removed } : undefined}
      animateDiffCount={Boolean(displayDiff)}
      diffCountActive={running}
      hideDiffCountWhenOpen
      actions={<ToolCardActions target={headerPath || argumentsText} output={tool.output} />}
      statusLabel={todoStatus ?? statusText(result, duration)}
      batchLabel={batchLabel}
      showFailureStatus={result?.tone === "danger" || tool.status === "failed"}
      isRunning={running}
      animateSummary={running}
      expanded={expanded}
      onToggle={() => setExpanded((value) => !value)}
    >
      {auditReason && (
        <div className="tool-permission-reason">
          <span>{t("Auto-audit reason", "自动审核理由")}</span>
          {auditReason}
        </div>
      )}
      {tool.name === "todo" ? (
        <ToolPanel className="todo-tool-view">
          <TodoToolView argumentsText={argumentsText} output={tool.output} />
        </ToolPanel>
      ) : (
        <ToolResultView name={tool.name} argumentsText={argumentsText} output={tool.output} headerPath={headerPath} />
      )}
    </ToolLayout>
  );
});

/**
 * 组装摘要行右段的状态文本。
 *
 * 结果摘要与耗时都可能缺席，全缺席时返回 undefined，
 * 让外壳跳过这一位而不是渲染空容器。
 *
 * @param result 文字摘要
 * @param duration 耗时文本
 * @returns 状态内容；无内容时返回 undefined
 */
function statusText(
  result: ReturnType<typeof toolResultSummary>,
  duration: string
): ReactNode {
  const label = result?.label ?? "";
  if (!label && !duration) return undefined;
  return (
    <span className="flex items-center gap-2">
      {label ? <span>{label}</span> : null}
      {duration ? <span className="tabular-nums">{duration}</span> : null}
    </span>
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
 * @param backgroundTask 是否为后台任务管理操作
 * @returns 可读名称
 */
export function readableToolName(name: string, backgroundTask = false): string {
  const labels: Record<string, string> = {
    run_command: "Shell",
    background_command: backgroundTask ? "Tasks" : "Shell",
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
