import { SyntaxHighlighter } from "../syntax-highlighter";
import { parseReadTextPages, type ReadTextPage } from "./read-result-parser";
import { prettyJson } from "./tool-data";
import { ToolFileReference } from "./tool-file-reference";
import { useI18n } from "../../i18n/use-i18n";

type ReadToolViewProps = {
  argumentsText: string;
  output: string;
  headerPath?: string;
};

/**
 * 渲染 read_file 单文件或批量文件结果。
 *
 * @param props argumentsText 为读取参数，output 为结果，headerPath 为卡片头部已展示路径
 * @returns 带行号和语法着色的文件读取详情
 */
export function ReadToolView({ output, headerPath }: ReadToolViewProps) {
  const pages = parseReadTextPages(output);
  if (pages.length === 0) {
    return output ? <pre className="generic-tool-block result"><code>{prettyJson(output)}</code></pre> : null;
  }
  return (
    <div className="read-tool-view">
      {pages.map((page, index) => (
        <ReadTextPageView page={page} hidePath={pages.length === 1 && page.path === headerPath} key={`${page.path}-${page.offset}-${index}`} />
      ))}
    </div>
  );
}

/**
 * 渲染一个文本分页。
 *
 * @param props page 为文本分页，hidePath 表示路径已经在工具卡头部展示
 * @returns 单文件内容块
 */
function ReadTextPageView({ page, hidePath }: { page: ReadTextPage; hidePath: boolean }) {
  const { t } = useI18n();
  const source = page.lines.map((line) => line.text).join("\n");
  return (
    <section className="read-file-page">
      {(!hidePath || page.offset !== null) && (
        <div className={`read-file-head${hidePath ? " path-hidden" : ""}`}>
          {!hidePath && <ToolFileReference path={page.path} />}
          {page.offset !== null && <small>{t(`Starting at line ${page.offset}`, `第 ${page.offset} 行起`)}</small>}
        </div>
      )}
      <div className="read-file-content">
        <div className="read-file-gutter" aria-hidden>
          {page.lines.map((line, index) => <span key={index}>{line.number ?? ""}</span>)}
        </div>
        <pre className="read-file-code"><SyntaxHighlighter language={languageOfPath(page.path)} source={source} /></pre>
      </div>
    </section>
  );
}

/**
 * 从文件路径推断语法着色语言。
 *
 * @param path 文件路径
 * @returns 文件扩展名，无扩展名时返回 undefined
 */
function languageOfPath(path: string): string | undefined {
  const name = path.split("/").pop() ?? "";
  return name.includes(".") ? name.split(".").pop() : undefined;
}
