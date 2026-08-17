import { FileCheck2 } from "lucide-react";
import { DiffView } from "./diff-view";
import { InlineDiffPreview } from "./layout/inline-diff-preview";
import { parseJsonRecord, prettyJson, stringField } from "./tool-data";
import { ToolFileReference } from "./tool-file-reference";
import { useI18n } from "../../i18n/use-i18n";

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
  const { t } = useI18n();
  const args = parseJsonRecord(argumentsText);
  const result = parseJsonRecord(output);
  const patch = stringField(args, "patch");
  const path = stringField(args, "path");
  const content = stringField(args, "content");
  const oldString = stringField(args, "old_string");
  const newString = stringField(args, "new_string");
  const changedFiles = Array.isArray(result?.changed_files) ? result.changed_files as ChangedFile[] : [];
  const outputDiff = result ? stringField(result, "diff") : "";
  const syntheticPatch = !outputDiff && !patch && path && (oldString || content)
    ? buildSyntheticPatch(path, oldString, newString || content, Boolean(content) && !oldString)
    : "";
  const chosen = outputDiff || patch || syntheticPatch;
  return (
    <div className="edit-tool-view">
      {!chosen && changedFiles.length > 0 && (
        <div className="changed-file-list">
          {changedFiles.map((file, index) => (
            <div className="changed-file" key={`${file.path}-${index}`}>
              <FileCheck2 size={14} />
              <span>
                {file.path && file.path !== headerPath && <ToolFileReference path={file.path} icon={false} />}
                {!file.path && <strong>{t("Unknown file", "未知文件")}</strong>}
                <small>{file.action || t("Edited", "已编辑")}</small>
              </span>
              <span className="changed-file-stats"><b>+{file.added ?? 0}</b><i>-{file.removed ?? 0}</i></span>
            </div>
          ))}
        </div>
      )}
      {chosen
        ? (
          <InlineDiffPreview>
            <DiffView source={chosen} headerPath={headerPath || path} />
          </InlineDiffPreview>
        )
        // 参数仍是未闭合 JSON 时不展示 `{...` 碎片；等 diff/完整参数就绪再渲染
        : args && argumentsText ? <pre className="generic-tool-block"><code>{prettyJson(argumentsText)}</code></pre> : null}
      {output && !result && <pre className={`generic-tool-block result${/^tool error:/i.test(output.trimStart()) ? " tool-error-output" : ""}`}><code>{output}</code></pre>}
    </div>
  );
}

/**
 * 统计补丁中以指定前缀开头的行数。
 *
 * @param source 补丁文本
 * @param prefix 行首字符
 * @returns 匹配行数
 */
function countPrefix(source: string, prefix: string): number {
  return source.split("\n").filter((line) => line.startsWith(prefix)).length;
}

/**
 * 将 write_file / str_replace 参数转成可预览的简易 patch。
 *
 * @param path 文件路径
 * @param oldText 旧文本
 * @param newText 新文本
 * @param isAdd 是否整文件新增/覆盖展示为 Add
 * @returns Codex 风格 patch 文本
 */
function buildSyntheticPatch(path: string, oldText: string, newText: string, isAdd: boolean): string {
  if (isAdd) {
    const body = newText.split("\n").map((line) => `+${line}`).join("\n");
    return `*** Begin Patch\n*** Add File: ${path}\n${body}\n*** End Patch`;
  }
  const removed = oldText ? oldText.split("\n").map((line) => `-${line}`).join("\n") : "";
  const added = newText.split("\n").map((line) => `+${line}`).join("\n");
  return `*** Begin Patch\n*** Update File: ${path}\n@@\n${removed}${removed && added ? "\n" : ""}${added}\n*** End Patch`;
}
