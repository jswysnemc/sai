import type { ToolLifecycle } from "../run-event-reducer";
import { readableToolName } from "../tool-lifecycle-card";
import { toolCardSummary } from "../tool-renderers/tool-card-summary";
import { parseJsonRecord, stringField, toolFilePath } from "../tool-renderers/tool-data";
import { displayPath, isCommandToolName } from "../tool-renderers/tool-display-summary";
import { ToolFileReference } from "../tool-renderers/tool-file-reference";
import { useI18n } from "../../i18n/use-i18n";

/** 轻量列表的单行数据 */
type ExploreRow = {
  /** 去重与渲染键 */
  key: string;
  /** 动作短词，如 Read / Search / Shell */
  action: string;
  /** 可在编辑器中打开的路径；无路径时为空串 */
  path: string;
  /** 行内展示文本 */
  label: string;
};

/**
 * 渲染纯探索工具组的展开态：一行一个对象的轻量清单。
 *
 * 探索操作没有值得逐卡审查的输出差异，展开时铺一排完整工具卡
 * 只会放大噪音；这里压成「动作 + 对象」行，路径行保持可点击打开。
 *
 * @param props tools 为组内探索项，workspacePath 用于展示相对路径
 * @returns 轻量文件行列表
 */
export function ExploreFileList({
  tools,
  workspacePath
}: {
  tools: ToolLifecycle[];
  workspacePath: string;
}) {
  const { locale } = useI18n();
  const rows = exploreRows(tools, locale, workspacePath);
  return (
    <ul className="explore-file-list">
      {rows.map((row) => (
        <li key={row.key} title={row.label}>
          <span className="explore-file-action">{row.action}</span>
          {row.path ? (
            <ToolFileReference path={row.path} label={row.label} className="explore-file-target" />
          ) : (
            <span className="explore-file-plain">{row.label}</span>
          )}
        </li>
      ))}
    </ul>
  );
}

/**
 * 将探索项压成去重后的展示行。
 *
 * 步骤:
 * 1. 逐项提取动作词与对象——路径优先，只读命令展示完整命令文本
 * 2. 以「动作 + 对象」为键去重，重复读取同一文件只留一行
 *
 * @param tools 组内探索项
 * @param locale 界面语言
 * @param workspacePath 当前工作区路径
 * @returns 保持首次出现顺序的行列表
 */
function exploreRows(
  tools: ToolLifecycle[],
  locale: ReturnType<typeof useI18n>["locale"],
  workspacePath: string
): ExploreRow[] {
  const seen = new Set<string>();
  const rows: ExploreRow[] = [];
  for (const tool of tools) {
    const argumentsText = tool.arguments || tool.argumentsPreview;
    const action = readableToolName(tool.name);
    const path = toolFilePath(tool.name, argumentsText);
    const label = path
      ? displayPath(path, workspacePath)
      : commandText(tool, argumentsText) || toolCardSummary(tool.name, argumentsText, locale, workspacePath);
    if (!label) continue;
    const key = `${action}:${label.toLowerCase()}`;
    if (seen.has(key)) continue;
    seen.add(key);
    rows.push({ key, action, path, label });
  }
  return rows;
}

/**
 * 提取命令类探索项的完整命令文本。
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
