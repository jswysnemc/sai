import { DiffView } from "./diff-view";
import { parseJsonRecord, stringField } from "./tool-data";
import { useI18n } from "../../i18n/use-i18n";

type ShellToolViewProps = {
  argumentsText: string;
  output: string;
};

/**
 * 渲染 Shell 命令、退出码、标准输出和错误输出。
 *
 * @param props 工具参数与结果
 * @returns 终端风格工具结果
 */
export function ShellToolView({ argumentsText, output }: ShellToolViewProps) {
  const { t } = useI18n();
  const args = parseJsonRecord(argumentsText);
  const result = parseJsonRecord(output);
  const command = stringField(args, "command") || argumentsText;
  const stdout = stringField(result, "stdout");
  const stderr = stringField(result, "stderr");
  const exitCode = typeof result?.exit_code === "number" ? result.exit_code : null;
  const success = typeof result?.success === "boolean" ? result.success : exitCode === 0;
  const diffOutput = isDiffCommand(command, stdout);
  return (
    <div className="shell-tool-view">
      <div className="shell-command-line"><span>$</span><code>{command}</code></div>
      {result && (
        <div className={success ? "shell-exit success" : "shell-exit failed"}>
          {success ? "ok" : `err (${exitCode ?? t("Unknown", "未知")})`}
        </div>
      )}
      {stdout && (diffOutput ? <DiffView source={stdout} /> : <pre className="shell-output"><code>{stdout}</code></pre>)}
      {stderr && <pre className="shell-output stderr"><code>{stderr}</code></pre>}
      {!result && output && <pre className="shell-output"><code>{output}</code></pre>}
    </div>
  );
}

/**
 * 判断命令结果是否应按 Diff 展示。
 *
 * @param command Shell 命令
 * @param stdout 标准输出
 * @returns 是否为 Diff 内容
 */
function isDiffCommand(command: string, stdout: string): boolean {
  return /(^|\s)git\s+(diff|show)\b/.test(command)
    || stdout.startsWith("diff --git")
    || stdout.includes("\n@@ ");
}
