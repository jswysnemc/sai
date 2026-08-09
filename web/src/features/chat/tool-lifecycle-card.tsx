import { ShieldCheck } from "lucide-react";
import type { ReactNode } from "react";
import { useQuery } from "@tanstack/react-query";
import { api } from "../../api/client";
import { usePersistedExpand } from "./message/tool-expand-state";
import type { ToolLifecycle } from "./run-event-reducer";
import { toolCardSummary } from "./tool-renderers/tool-card-summary";
import { parseCodexSubagentActivity } from "./tool-renderers/codex-subagent-data";
import { CodexSubagentToolView } from "./tool-renderers/codex-subagent-tool-view";
import { parseJsonRecord, stringField, toolFilePath } from "./tool-renderers/tool-data";
import { AnimatedCount } from "./tool-renderers/animated-count";
import { streamedLineCount } from "./tool-renderers/write-progress";
import { ToolCardActions } from "./tool-renderers/tool-card-actions";
import { ToolFileReference } from "./tool-renderers/tool-file-reference";
import { displayPath, isCommandToolName, isEditToolName } from "./tool-renderers/tool-display-summary";
import { toolDiffStat, toolResultSummary } from "./tool-renderers/tool-result-summary";
import { toolDurationLabel } from "./tool-renderers/tool-duration";
import { useElapsedClock } from "./tool-renderers/use-elapsed-clock";
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
  // 执行中的卡片需要推进计时；结束后停表，历史卡片不占用任何定时器
  const running = tool.status === "preparing" || tool.status === "running";
  const now = useElapsedClock(running);
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
  // 3. 操作对象：命令类展示完整命令占满剩余宽度，放不下才截断；
  //    其余优先取工作区相对路径，再退回参数摘要
  const isCommand = isCommandToolName(tool.name);
  const fullCommand = isCommand
    ? stringField(parseJsonRecord(argumentsText), "command")
      || stringField(parseJsonRecord(argumentsText), "cmd")
    : "";
  const relativePath = headerPath ? displayPath(headerPath, workspacePath) : "";
  const summary = headerPath
    ? ""
    : fullCommand || toolCardSummary(tool.name, argumentsText, locale, workspacePath) || tool.progress;
  const target = headerPath
    ? <ToolFileReference path={headerPath} label={relativePath || headerPath} className="tool-shell-file" icon={false} />
    : summary;
  // 展开后详情区已有完整的 `$ 命令` 行，头部再放一遍就是冗余信息
  const hideTarget = expanded && isCommand && Boolean(fullCommand);

  // 4. 权限已并入本卡：头部只留一枚徽章，理由放进展开区，不再单独占一张卡
  const permission = tool.permission;
  const autoAudited = permission?.decision === "allow" && permission.source === "auto_audit";
  const auditReason = permission?.decision === "allow" ? permission.reason?.trim() ?? "" : "";

  // 5. 折叠行右段依次表达"结果如何"与"花了多久"；
  //    编辑类流式期间先展示滚动的已写入行数，结束后换成增删统计
  const result = toolResultSummary(tool.name, tool.output, locale);
  const diffStat = toolDiffStat(tool.name, tool.output);
  const duration = toolDurationLabel(tool.startedAtMs, tool.endedAtMs, now);
  const writingLines = running && isEditToolName(tool.name)
    ? streamedLineCount(argumentsText)
    : 0;

  return (
    <ToolCardShell
      tone={toneOfState(tool.status)}
      icon={<ToolIcon name={tool.name} />}
      title={readableToolName(tool.name)}
      target={hideTarget ? undefined : target || undefined}
      targetTitle={hideTarget ? undefined : fullCommand || headerPath || summary || undefined}
      actions={<ToolCardActions target={copyableInput(argumentsText, headerPath)} output={tool.output} />}
      summary={
        writingLines > 0
          ? (
            <>
              {t("Writing", "写入")} <AnimatedCount value={writingLines} active={running} /> {t("lines", "行")}
            </>
          )
          : resultSummaryNode(result, diffStat)
      }
      summaryTone={result?.tone ?? "neutral"}
      meta={metaNode(permission ? <ToolPermissionBadge autoAudited={autoAudited} t={t} /> : null, duration)}
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
 * 组装折叠行的结果摘要内容。
 *
 * 编辑类工具的增删需要各自着色，因此与文字摘要分成两条路径，
 * 不压成一个字符串。两者都缺席时返回 undefined，外壳据此不渲染摘要位，
 * 避免在头部留下一个空节点撑开间距。
 *
 * @param result 文字摘要
 * @param diffStat 增删统计
 * @returns 摘要内容；无内容时返回 undefined
 */
function resultSummaryNode(
  result: ReturnType<typeof toolResultSummary>,
  diffStat: ReturnType<typeof toolDiffStat>
): ReactNode {
  if (diffStat) {
    return (
      <>
        {diffStat.added > 0 && <b>+{diffStat.added}</b>}
        {diffStat.removed > 0 && <i>-{diffStat.removed}</i>}
      </>
    );
  }
  return result?.label || undefined;
}

/**
 * 组装折叠行的元信息内容。
 *
 * 权限徽章与耗时都可能缺席，全缺席时返回 undefined，
 * 让外壳跳过这一位而不是渲染空容器。
 *
 * @param badge 权限徽章元素
 * @param duration 耗时文本
 * @returns 元信息内容；无内容时返回 undefined
 */
function metaNode(badge: ReactNode, duration: string): ReactNode {
  if (!badge && !duration) return undefined;
  return (
    <>
      {badge}
      {duration}
    </>
  );
}

/**
 * 选出适合复制的调用内容。
 *
 * 有明确文件路径时复制路径，否则退回原始参数文本——
 * 前者是用户接下来最可能粘到别处的东西，后者至少保证不丢信息。
 *
 * @param argumentsText 参数文本
 * @param headerPath 头部展示的文件路径
 * @returns 待复制文本
 */
function copyableInput(argumentsText: string, headerPath: string): string {
  return headerPath || argumentsText;
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
export function readableToolName(name: string): string {
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
