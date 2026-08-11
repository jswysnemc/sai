import { ChevronDown, FilePenLine, ListChecks, Search, ShieldCheck, TerminalSquare, Wrench } from "lucide-react";
import { useEffect, useRef } from "react";
import { useQuery } from "@tanstack/react-query";
import { api } from "../../../api/client";
import { Button } from "../../../shared/ui/button/button";
import { groupHasExpandedTool, usePersistedExpand } from "./tool-expand-state";
import type { ToolLifecycle } from "../run-event-reducer";
import { ToolLifecycleCard, readableToolName } from "../tool-lifecycle-card";
import { ExploreFileList } from "./explore-file-list";
import { ToolGroupTicker } from "./tool-group-ticker";
import { toolCardSummary } from "../tool-renderers/tool-card-summary";
import { toolFilePath } from "../tool-renderers/tool-data";
import { displayPath } from "../tool-renderers/tool-display-summary";
import {
  isCommandTool,
  isEditTool,
  isExploreTool,
  shouldUseLightweightExploreList,
  toolCallGroupLabel
} from "./tool-call-grouping";
import { useI18n } from "../../i18n/use-i18n";
import "./tool-call-group.css";

/**
 * 渲染默认折叠的连续已完成工具组。
 *
 * 实时轮次里折叠行做纵向轮播：新完成工具的摘要滚入，
 * 静默后落到「探索了 x 个文件」这类聚合标签。
 *
 * @param props tools 为组内工具调用，live 表示所属轮次是否仍在进行
 * @returns 工具组标题和可展开原始卡片
 */
export function ToolCallGroup({ tools, live = false }: { tools: ToolLifecycle[]; live?: boolean }) {
  const { locale, t } = useI18n();
  const groupRef = useRef<HTMLElement | null>(null);
  const workspaces = useQuery({ queryKey: ["workspaces"], queryFn: api.workspaces.list, staleTime: 30_000 });
  const workspacePath = workspaces.data?.workspaces.find((item) => item.id === workspaces.data?.active_id)?.path ?? "";
  // 组 id 用首项稳定；若用户已展开组内任一工具则保持展开
  const groupId = tools[0]?.id ? `tool-group-${tools[0].id}` : "tool-group";
  const [expanded, setExpanded] = usePersistedExpand(
    groupId,
    groupHasExpandedTool(tools.map((tool) => tool.id))
  );
  const todoOnly = tools.every((tool) => tool.name === "todo");
  const exploreOnly = tools.every(isExploreTool);
  // 含只读 Shell 的探索组不能用轻量清单，否则无法二次展开看命令输出
  const lightweightExplore = shouldUseLightweightExploreList(tools);
  const commandOnly = tools.every(isCommandTool);
  const editOnly = tools.every(isEditTool);
  const label = toolCallGroupLabel(tools, locale, workspacePath);
  // 每项工具压成「名称 对象」的单行摘要，供折叠行轮播
  const tickerItems = tools.map((tool) => toolTickerLabel(tool, locale, workspacePath));
  // 组内经过权限审核的项数：折叠态也要能看出这批操作动过权限
  const auditedCount = tools.filter((tool) => tool.permission).length;
  const icon = todoOnly
    ? <ListChecks size={14} />
    : exploreOnly
      ? <Search size={14} />
      : commandOnly
        ? <TerminalSquare size={14} />
        : editOnly
          ? <FilePenLine size={14} />
          : <Wrench size={14} />;
  return (
    <section ref={groupRef} className={`tool-call-group${expanded ? " expanded" : ""}`}>
      <Button className="tool-call-group-trigger" onClick={() => setExpanded((value) => !value)} aria-expanded={expanded}>
        <span className="tool-call-group-icon">{icon}</span>
        <strong title={label}>
          <ToolGroupTicker items={tickerItems} aggregate={label} live={live && !expanded} />
        </strong>
        {auditedCount > 0 && (
          <span
            className="tool-call-group-audited"
            title={t(`${auditedCount} of them required a permission decision`, `其中 ${auditedCount} 项经过权限审核`)}
          >
            <ShieldCheck size={11} aria-hidden />
            {auditedCount}
          </span>
        )}
        <ChevronDown size={14} className={expanded ? "rotate" : ""} aria-hidden />
      </Button>
      {expanded && (
        lightweightExplore ? (
          // 纯文件探索：轻量清单足够；含 Shell 时改走完整卡片以便二次展开
          <ExploreFileList tools={tools} workspacePath={workspacePath} />
        ) : (
          <div className="tool-call-group-items">
            {tools.map((tool) => <ToolLifecycleCard key={tool.id} tool={tool} />)}
          </div>
        )
      )}
    </section>
  );
}

/**
 * 组装单项工具的轮播摘要行。
 *
 * 与工具卡头部同一套信息：可读名称加操作对象，
 * 对象优先取工作区相对路径，其次是参数摘要。
 *
 * @param tool 工具调用
 * @param locale 界面语言
 * @param workspacePath 当前工作区路径
 * @returns 单行摘要
 */
function toolTickerLabel(
  tool: ToolLifecycle,
  locale: ReturnType<typeof useI18n>["locale"],
  workspacePath: string
): string {
  const argumentsText = tool.arguments || tool.argumentsPreview;
  const headerPath = toolFilePath(tool.name, argumentsText);
  const target = headerPath
    ? displayPath(headerPath, workspacePath)
    : toolCardSummary(tool.name, argumentsText, locale, workspacePath);
  const name = readableToolName(tool.name);
  return target ? `${name} ${target}` : name;
}
