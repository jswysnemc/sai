import { ChevronDown, FilePenLine, ListChecks, Search, ShieldCheck, TerminalSquare, Wrench } from "lucide-react";
import { useRef } from "react";
import { useQuery } from "@tanstack/react-query";
import { api } from "../../../api/client";
import { Button } from "../../../shared/ui/button/button";
import { usePersistedExpand } from "./tool-expand-state";
import type { ToolLifecycle } from "../run-event-reducer";
import { readableToolName } from "../tool-lifecycle-card";
import { ToolRowList } from "./tool-row-list";
import { ToolGroupTicker } from "./tool-group-ticker";
import { toolCardSummary } from "../tool-renderers/tool-card-summary";
import { toolFilePath } from "../tool-renderers/tool-data";
import { displayPath } from "../tool-renderers/tool-display-summary";
import {
  isCommandTool,
  isEditTool,
  isExploreTool,
  toolCallGroupTitle
} from "./tool-call-grouping";
import { useI18n } from "../../i18n/use-i18n";
import "./tool-call-group.css";

/**
 * 渲染连续已完成工具组：短标题 + 常驻条目清单。
 *
 * 参考 zcode 排版：组头是「探索 · 2 文件」式分类计数，条目默认可见，
 * 每行「已读取 / 已执行 + 对象」，有输出的行可单独展开详情。
 * 实时轮次里组头做纵向轮播，新完成工具的摘要滚入。
 *
 * @param props tools 为组内工具调用，live 表示所属轮次是否仍在进行
 * @returns 工具组标题和条目清单
 */
export function ToolCallGroup({ tools, live = false }: { tools: ToolLifecycle[]; live?: boolean }) {
  const { locale, t } = useI18n();
  const groupRef = useRef<HTMLElement | null>(null);
  const workspaces = useQuery({ queryKey: ["workspaces"], queryFn: api.workspaces.list, staleTime: 30_000 });
  const workspacePath = workspaces.data?.workspaces.find((item) => item.id === workspaces.data?.active_id)?.path ?? "";
  // 组 id 用首项稳定；条目清单默认可见，用户收起后按组记忆
  const groupId = tools[0]?.id ? `tool-group-${tools[0].id}` : "tool-group";
  const [expanded, setExpanded] = usePersistedExpand(groupId, true);
  const todoOnly = tools.every((tool) => tool.name === "todo");
  const exploreOnly = tools.every(isExploreTool);
  const commandOnly = tools.every(isCommandTool);
  const editOnly = tools.every(isEditTool);
  const title = toolCallGroupTitle(tools, locale, workspacePath);
  // 每项工具压成「名称 对象」的单行摘要，供进行中的组头轮播
  const tickerItems = tools.map((tool) => toolTickerLabel(tool, locale, workspacePath));
  // 组内经过权限审核的项数：收起态也要能看出这批操作动过权限
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
        <strong title={title}>
          <ToolGroupTicker items={tickerItems} aggregate={title} live={live && !expanded} />
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
      {expanded && <ToolRowList tools={tools} workspacePath={workspacePath} />}
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
