import { SyntaxHighlighter } from "../syntax-highlighter";
import { CollapsibleOutput } from "./collapsible-output";
import { ToolPanel } from "./layout/tool-panel";
import { prettyJson } from "./tool-data";
import { parseToolFields } from "./tool-fields";
import { useI18n } from "../../i18n/use-i18n";

type GenericToolViewProps = {
  argumentsText: string;
  output: string;
};

/**
 * 渲染未专门适配工具的参数和结果。
 *
 * 参数按字段拆开展示而不是倾倒整段 JSON：未适配的工具本来就缺少语义线索，
 * 再让读者自己在花括号里配对键值，展开这张卡就没有意义。
 *
 * @param props 工具参数与输出
 * @returns 通用工具详情
 */
export function GenericToolView({ argumentsText, output }: GenericToolViewProps) {
  const { t } = useI18n();
  const fields = parseToolFields(argumentsText);
  return (
    <ToolPanel className="generic-tool-view">
      {argumentsText && (
        <section>
          <span>{t("Arguments", "参数")}</span>
          {fields.length > 0
            ? <ToolFieldList fields={fields} />
            : <JsonBlock source={argumentsText} />}
        </section>
      )}
      {output && <section><span>{t("Result", "结果")}</span><JsonBlock source={output} className="result" /></section>}
    </ToolPanel>
  );
}

/**
 * 渲染字段级参数列表。
 *
 * 短值与键名同行、长值另起一块，因此扫一眼就能看清有哪些参数，
 * 需要细读某个长文本时它也不会被挤成一条难读的窄列。
 *
 * @param props fields 为已拆解的字段列表
 * @returns 参数字段列表
 */
function ToolFieldList({ fields }: { fields: ReturnType<typeof parseToolFields> }) {
  return (
    <dl className="tool-field-list">
      {fields.map((field) => (
        <div className={field.block ? "tool-field is-block" : "tool-field"} key={field.key}>
          <dt>{field.key}</dt>
          <dd>{field.block ? <pre>{field.value}</pre> : field.value}</dd>
        </div>
      ))}
    </dl>
  );
}

/**
 * 渲染格式化文本块，内容为合法 JSON 时做语法着色。
 *
 * @param props 原始文本与附加类名
 * @returns 着色或纯文本代码块
 */
export function JsonBlock({ source, className = "" }: { source: string; className?: string }) {
  const pretty = prettyJson(source);
  const isJson = pretty !== source || source.trimStart().startsWith("{") || source.trimStart().startsWith("[");
  const errorClass = /^tool error:/i.test(source.trimStart()) ? "tool-error-output" : "";
  // 纯文本输出可能很长，交给折叠渲染；JSON 已经过格式化，保持整块着色
  if (!isJson) {
    return <CollapsibleOutput source={pretty} className={`generic-tool-block ${className} ${errorClass}`.trim()} />;
  }
  return (
    <pre className={`generic-tool-block ${className} ${errorClass}`.trim()}>
      <SyntaxHighlighter language="json" source={pretty} />
    </pre>
  );
}
