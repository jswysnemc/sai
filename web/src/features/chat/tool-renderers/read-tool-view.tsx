import { SyntaxHighlighter } from "../syntax-highlighter";
import { ToolPanel } from "./layout/tool-panel";
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
    <ToolPanel className="read-tool-view">
      {pages.map((page, index) => (
        <ReadTextPageView page={page} hidePath={pages.length === 1 && page.path === headerPath} key={`${page.path}-${page.offset}-${index}`} />
      ))}
    </ToolPanel>
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
  const range = formatReadRange(page, t);
  return (
    <section className="read-file-page">
      {(!hidePath || range !== null) && (
        <div className={`read-file-head${hidePath ? " path-hidden" : ""}`}>
          {!hidePath && <ToolFileReference path={page.path} />}
          {range !== null && <small className="read-file-range">{range}</small>}
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
 * 组装读取范围说明。
 *
 * 优先展示实际覆盖的行区间；请求 limit 与实际行数不一致时补充说明，
 * 因截断而提前结束时给出续读起点。
 *
 * @param page 文本分页
 * @param t 双语文本选择方法
 * @returns 范围说明文本，无可展示信息时返回 null
 */
function formatReadRange(page: ReadTextPage, t: (en: string, zh: string) => string): string | null {
  const parts: string[] = [];
  const start = page.offset;
  if (start !== null && page.lineCount > 0) {
    const end = start + page.lineCount - 1;
    parts.push(t(`L${start}–L${end}`, `第 ${start}–${end} 行`));
  } else if (start !== null) {
    parts.push(t(`From L${start}`, `第 ${start} 行起`));
  }
  if (page.lineCount > 0) {
    parts.push(t(`${page.lineCount} lines`, `${page.lineCount} 行`));
  }
  // 请求上限与实际返回不一致时说明差异来源
  if (page.limit !== null && page.limit !== page.lineCount) {
    parts.push(t(`limit ${page.limit}`, `上限 ${page.limit}`));
  }
  if (page.truncated) {
    parts.push(
      page.next !== null
        ? t(`truncated, resume at L${page.next}`, `已截断，续读第 ${page.next} 行`)
        : t("truncated", "已截断")
    );
  }
  return parts.length > 0 ? parts.join(" · ") : null;
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
