import { FileCheck2 } from "lucide-react";
import { DiffView } from "./diff-view";
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
                <small>{file.action || t("Edited", "已编辑")}</small>
              </span>
              <span className="changed-file-stats"><b>+{file.added ?? 0}</b><i>-{file.removed ?? 0}</i></span>
            </div>
          ))}
        </div>
      )}
      {patch ? <DiffView source={patch} headerPath={headerPath} /> : argumentsText && <pre className="generic-tool-block"><code>{prettyJson(argumentsText)}</code></pre>}
      {output && !result && <pre className={`generic-tool-block result${/^tool error:/i.test(output.trimStart()) ? " tool-error-output" : ""}`}><code>{output}</code></pre>}
    </div>
  );
}
