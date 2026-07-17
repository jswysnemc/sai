import { EditToolView } from "../chat/tool-renderers/edit-tool-view";
import { ShellToolView } from "../chat/tool-renderers/shell-tool-view";
import { parseJsonRecord } from "../chat/tool-renderers/tool-data";
import { useI18n } from "../i18n/use-i18n";

type PermissionArgumentDetailsProps = {
  tool: string;
  argumentsText: string;
};

/**
 * 按工具语义渲染权限请求参数，不直接暴露 JSON 对象。
 *
 * @param props 工具名称和参数文本
 * @returns 命令、Diff 或键值详情
 */
export function PermissionArgumentDetails({ tool, argumentsText }: PermissionArgumentDetailsProps) {
  const { t } = useI18n();
  if (tool === "run_command" || tool.includes("background_command")) {
    return <ShellToolView argumentsText={argumentsText} output="" />;
  }
  if (tool === "edit_file" || tool === "apply_patch") {
    return <EditToolView argumentsText={argumentsText} output="" />;
  }
  const fields = semanticFields(argumentsText, t);
  if (fields.length === 0) return null;
  return (
    <dl className="permission-argument-list">
      {fields.map((field) => (
        <div key={field.key}>
          <dt>{field.label}</dt>
          <dd>{field.value}</dd>
        </div>
      ))}
    </dl>
  );
}

type SemanticField = {
  key: string;
  label: string;
  value: string;
};

/**
 * 将通用工具参数转换为紧凑键值列表。
 *
 * @param argumentsText 工具参数 JSON 文本
 * @param t 双语文本选择方法
 * @returns 可直接展示的语义字段
 */
function semanticFields(argumentsText: string, t: (en: string, zh: string) => string): SemanticField[] {
  const record = parseJsonRecord(argumentsText);
  if (!record) {
    const value = argumentsText
      .trim()
      .replace(/^\{+|\}+$/g, "")
      .replaceAll("\\n", "\n")
      .replaceAll("\\t", "\t")
      .replaceAll("\\\"", "\"")
      .replaceAll("\"", "");
    return value ? [{ key: "arguments", label: t("Arguments", "参数"), value }] : [];
  }
  return Object.entries(record)
    .filter(([key, value]) => !key.startsWith("_sai_") && value !== null && value !== "")
    .map(([key, value]) => ({
      key,
      label: fieldLabel(key, t),
      value: displayValue(value, t)
    }))
    .filter((field) => field.value.length > 0);
}

/**
 * 将未知参数值转换为不含 JSON 外框的文本。
 *
 * @param value 参数值
 * @param t 双语文本选择方法
 * @returns 可读文本
 */
function displayValue(value: unknown, t: (en: string, zh: string) => string): string {
  if (Array.isArray(value)) return value.map((item) => displayValue(item, t)).filter(Boolean).join("\n");
  if (value !== null && typeof value === "object") {
    return Object.entries(value as Record<string, unknown>)
      .map(([key, nested]) => `${fieldLabel(key, t)}: ${displayValue(nested, t)}`)
      .join("\n");
  }
  return String(value).slice(0, 1600);
}

/**
 * 返回常用参数字段的中文标签。
 *
 * @param key 参数字段名
 * @param t 双语文本选择方法
 * @returns 用户可读标签
 */
function fieldLabel(key: string, t: (en: string, zh: string) => string): string {
  return {
    command: t("Command", "命令"),
    cwd: t("Working directory", "工作目录"),
    path: t("File", "文件"),
    file: t("File", "文件"),
    target: t("Target", "目标"),
    destination: t("Destination", "目标位置"),
    query: t("Query", "查询"),
    task: t("Task", "任务"),
    description: t("Description", "说明"),
    content: t("Content", "内容"),
    patch: t("Changes", "变更")
  }[key] ?? key.replaceAll("_", " ");
}
