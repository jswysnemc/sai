import { FileCheck2 } from "lucide-react";
import { DiffView } from "./diff-view";
import { parseJsonRecord, prettyJson, stringField } from "./tool-data";
import { ToolFileReference } from "./tool-file-reference";
import { useI18n } from "../../i18n/use-i18n";
import { text, type Locale } from "../../i18n/locale";

type EditToolViewProps = {
  argumentsText: string;
  output: string;
  headerPath?: string;
};

type ChangedFile = {
  path?: string;
  action?: string;
  added?: number;
  removed?: number;
};

/**
 * 渲染文件修改 patch、变更文件和行数统计。
 *
 * @param props 编辑参数与结果
 * @returns 文件修改详情
 */
export function EditToolView({ argumentsText, output, headerPath }: EditToolViewProps) {
  const { locale, t } = useI18n();
  const args = parseJsonRecord(argumentsText);
  const result = parseJsonRecord(output);
  const patch = stringField(args, "patch") || legacyEditAsPatch(args, locale);
  const path = stringField(args, "path");
  const changedFiles = Array.isArray(result?.changed_files) ? result.changed_files as ChangedFile[] : [];
  return (
    <div className="edit-tool-view">
      {!patch && changedFiles.length > 0 && (
        <div className="changed-file-list">
          {changedFiles.map((file, index) => (
            <div className="changed-file" key={`${file.path}-${index}`}>
              <FileCheck2 size={14} />
              <span>
                {file.path && file.path !== headerPath && <ToolFileReference path={file.path} icon={false} />}
                {!file.path && <strong>{t("Unknown file", "未知文件")}</strong>}
                <small>{file.action || "Edited"}</small>
              </span>
              <span className="changed-file-stats"><b>+{file.added ?? 0}</b><i>-{file.removed ?? 0}</i></span>
            </div>
          ))}
        </div>
      )}
      {path && path !== headerPath && !patch && <div className="legacy-edit-path"><span>{t("File", "文件")}</span><ToolFileReference path={path} icon={false} /></div>}
      {patch ? <DiffView source={patch} headerPath={headerPath} /> : argumentsText && <pre className="generic-tool-block"><code>{prettyJson(argumentsText)}</code></pre>}
      {output && !result && <pre className={`generic-tool-block result${/^tool error:/i.test(output.trimStart()) ? " tool-error-output" : ""}`}><code>{output}</code></pre>}
    </div>
  );
}

/**
 * 把旧编辑模式参数转换为可展示的 patch 文本。
 *
 * @param args 编辑工具参数
 * @returns Codex 风格 patch 文本，无法转换时返回空串
 */
function legacyEditAsPatch(args: Record<string, unknown> | null, locale: Locale): string {
  const path = stringField(args, "path");
  if (!path) return "";
  const content = stringField(args, "content");
  if (content) {
    const body = content.replace(/\n$/, "").split("\n").map((line) => `+${line}`).join("\n");
    return `*** Update File: ${path}\n${body}`;
  }
  const replacement = args && typeof args.replacement === "string" ? args.replacement : null;
  const start = args && typeof args.start_line === "number" ? args.start_line : null;
  const end = args && typeof args.end_line === "number" ? args.end_line : null;
  if (replacement === null || start === null || end === null) return "";
  const body = replacement.replace(/\n$/, "").split("\n").map((line) => `+${line}`).join("\n");
  return `*** Update File: ${path}\n@@ ${text(locale, `Lines ${start}-${end}`, `第 ${start}-${end} 行`)}\n${replacement ? body : text(locale, "-(delete this line range)", "-（删除该行范围）")}`;
}
