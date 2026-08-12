import { ChevronDown, ShieldCheck } from "lucide-react";
import type { ToolLifecycle } from "../run-event-reducer";
import { usePersistedExpand } from "./tool-expand-state";
import { toolCardSummary } from "../tool-renderers/tool-card-summary";
import { parseJsonRecord, stringField, toolFilePath } from "../tool-renderers/tool-data";
import { displayPath, isCommandToolName } from "../tool-renderers/tool-display-summary";
import { toolDiffStat } from "../tool-renderers/tool-result-summary";
import { ToolFileReference } from "../tool-renderers/tool-file-reference";
import { ToolResultView } from "../tool-renderers/tool-result-view";
import { TodoToolView } from "../tool-renderers/todo-tool-view";
import { toolDoneVerb } from "./tool-call-grouping";
import { useI18n } from "../../i18n/use-i18n";

/** 清单的单行数据 */
type ToolRowData = {
  /** 渲染键：可展开行用调用 id，纯文件行用对象去重键 */
  key: string;
  /** 源工具调用 */
  tool: ToolLifecycle;
  /** 完成态动词，如「已读取」 */
  verb: string;
  /** 可在编辑器中打开的路径；无路径时为空串 */
  path: string;
  /** 行内展示文本（文件名或命令全文） */
  label: string;
  /** 文件行的父目录后缀（弱化展示） */
  directory: string;
  /** 是否为命令，命令用等宽字体 */
  command: boolean;
  /** 是否有可展开的详情（输出 / diff） */
  expandable: boolean;
};

/**
 * 渲染工具组的常驻条目清单：一行一个操作，参考 zcode 排版。
 *
 * 行结构为「完成态动词 + 对象」，文件对象带类型图标并可点击打开，
 * 命令等宽展示；有输出或 diff 的行可单独展开完整详情视图。
 *
 * @param props tools 为组内完成项，workspacePath 用于展示相对路径
 * @returns 工具条目清单
 */
export function ToolRowList({
  tools,
  workspacePath
}: {
  tools: ToolLifecycle[];
  workspacePath: string;
}) {
  const { locale } = useI18n();
  const rows = toolRows(tools, locale, workspacePath);
  return (
    <ul className="tool-row-list">
      {rows.map((row) => (
        <ToolRow key={row.key} row={row} />
      ))}
    </ul>
  );
}

/**
 * 渲染单行操作，可展开行内嵌详情视图。
 *
 * @param props row 为该行数据
 * @returns 清单行
 */
function ToolRow({ row }: { row: ToolRowData }) {
  const { t } = useI18n();
  const [open, setOpen] = usePersistedExpand(row.tool.id, false);
  const expanded = row.expandable && open;
  const diff = toolDiffStat(row.tool.name, row.tool.output);
  const body = (
    <>
      <span className="tool-row-verb">{row.verb}</span>
      {row.path ? (
        <span className="tool-row-target">
          <ToolFileReference path={row.path} label={row.label} className="tool-row-file" />
          {row.directory && <em className="tool-row-directory">{row.directory}</em>}
        </span>
      ) : (
        // 展开后详情区已有完整命令块，行内不再重复命令全文
        <span
          className={`tool-row-target tool-row-plain${row.command ? " is-command" : ""}`}
        >
          {expanded && row.command ? "" : row.label}
        </span>
      )}
      {diff && (
        <span className="tool-row-diff">
          <em className="added">+{diff.added}</em>
          <em className="removed">-{diff.removed}</em>
        </span>
      )}
      {row.tool.permission && (
        <span
          className="tool-row-audited"
          title={t("This call required a permission decision", "该调用经过权限审核")}
        >
          <ShieldCheck size={11} aria-hidden />
        </span>
      )}
      {row.expandable && (
        <ChevronDown size={12} className={`tool-row-chevron${expanded ? " rotate" : ""}`} aria-hidden />
      )}
    </>
  );
  return (
    <li className={expanded ? "tool-row expanded" : "tool-row"} title={row.label}>
      {row.expandable ? (
        <button
          type="button"
          className="tool-row-line"
          onClick={() => setOpen((value) => !value)}
          aria-expanded={expanded}
        >
          {body}
        </button>
      ) : (
        <span className="tool-row-line">{body}</span>
      )}
      {expanded && (
        <div className="tool-row-detail">
          {row.tool.name === "todo" ? (
            <TodoToolView
              toolId={row.tool.id}
              argumentsText={row.tool.arguments || row.tool.argumentsPreview}
              output={row.tool.output}
            />
          ) : (
            <ToolResultView
              name={row.tool.name}
              argumentsText={row.tool.arguments || row.tool.argumentsPreview}
              output={row.tool.output}
              headerPath={row.path || undefined}
            />
          )}
        </div>
      )}
    </li>
  );
}

/**
 * 将工具调用压成展示行。
 *
 * 步骤:
 * 1. 逐项提取动词与对象——路径优先拆分文件名与父目录，命令展示全文
 * 2. 无详情的纯文件行按「动词 + 对象」去重；可展开的行保留每次调用
 *
 * @param tools 组内工具项
 * @param locale 界面语言
 * @param workspacePath 当前工作区路径
 * @returns 保持首次出现顺序的行列表
 */
function toolRows(
  tools: ToolLifecycle[],
  locale: ReturnType<typeof useI18n>["locale"],
  workspacePath: string
): ToolRowData[] {
  const seen = new Set<string>();
  const rows: ToolRowData[] = [];
  for (const tool of tools) {
    const argumentsText = tool.arguments || tool.argumentsPreview;
    const verb = toolDoneVerb(tool.name, locale);
    const path = toolFilePath(tool.name, argumentsText);
    const command = commandText(tool, argumentsText);
    const relative = path ? displayPath(path, workspacePath) : "";
    const { name: fileName, directory } = splitPathLabel(relative);
    const label = path
      ? fileName
      : command || toolCardSummary(tool.name, argumentsText, locale, workspacePath) || tool.name;
    const expandable = hasExpandableDetail(tool);
    if (!expandable) {
      // 纯展示行（读取过的文件等）重复出现没有额外信息，按对象去重
      const dedupeKey = `${verb}:${relative || label}`.toLowerCase();
      if (seen.has(dedupeKey)) continue;
      seen.add(dedupeKey);
      rows.push({
        key: dedupeKey,
        tool,
        verb,
        path,
        label,
        directory,
        command: Boolean(command) && !path,
        expandable
      });
      continue;
    }
    rows.push({
      key: tool.id,
      tool,
      verb,
      path,
      label,
      directory,
      command: Boolean(command) && !path,
      expandable
    });
  }
  return rows;
}

/**
 * 判断行是否有值得展开的详情。
 *
 * 命令输出、编辑 diff、todo 快照与其他有输出的调用都可展开；
 * 纯文件读取的正文已能在编辑器打开，不再内嵌。
 *
 * @param tool 工具生命周期
 * @returns 可展开时返回 true
 */
function hasExpandableDetail(tool: ToolLifecycle): boolean {
  if (tool.name === "read_file" || tool.name === "grep" || tool.name === "glob" || tool.name === "list_dir") {
    return false;
  }
  if (tool.name === "edit_file" || tool.name === "write_file" || tool.name === "str_replace") {
    return true;
  }
  return Boolean(tool.output.trim());
}

/**
 * 拆分相对路径为文件名与父目录。
 *
 * @param relative 工作区相对路径
 * @returns 文件名与弱化展示的目录后缀（含尾斜杠）
 */
function splitPathLabel(relative: string): { name: string; directory: string } {
  const trimmed = relative.replace(/\/+$/, "");
  const index = trimmed.lastIndexOf("/");
  if (index <= 0) return { name: trimmed || relative, directory: "" };
  return {
    name: trimmed.slice(index + 1),
    directory: `${trimmed.slice(0, index)}/`
  };
}

/**
 * 提取命令类工具的完整命令文本。
 *
 * @param tool 工具生命周期
 * @param argumentsText 参数文本
 * @returns 命令全文；非命令类返回空串
 */
function commandText(tool: ToolLifecycle, argumentsText: string): string {
  if (!isCommandToolName(tool.name)) return "";
  const args = parseJsonRecord(argumentsText);
  if (!args) return "";
  return stringField(args, "command") || stringField(args, "cmd");
}
