import { SyntaxHighlighter } from "../syntax-highlighter";
import { prettyJson } from "./tool-data";
import { useI18n } from "../../i18n/use-i18n";

type GenericToolViewProps = {
  argumentsText: string;
  output: string;
};

/**
 * 渲染未专门适配工具的参数和结果。
 *
 * @param props 工具参数与输出
 * @returns 通用工具详情
 */
export function GenericToolView({ argumentsText, output }: GenericToolViewProps) {
  const { t } = useI18n();
  return (
    <div className="generic-tool-view">
      {argumentsText && <section><span>{t("Arguments", "参数")}</span><JsonBlock source={argumentsText} /></section>}
      {output && <section><span>{t("Result", "结果")}</span><JsonBlock source={output} className="result" /></section>}
    </div>
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
  return (
    <pre className={`generic-tool-block ${className} ${errorClass}`.trim()}>
      {isJson ? <SyntaxHighlighter language="json" source={pretty} /> : <code>{pretty}</code>}
    </pre>
  );
}
