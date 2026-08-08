import { CollapsibleOutput } from "./collapsible-output";
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
 * 命令与输出之间加一条细分隔：两者都是等宽文本，不分隔时长命令与首行输出会连成一片。
 * 输出走折叠渲染并解析 ANSI 着色，因此长构建日志不会撑开整屏，
 * 编译器本来用颜色表达的错误分级也保留下来。
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
  const hasBody = Boolean(stdout || stderr || (!result && output));
  return (
    <div className="shell-tool-view">
      <div className={`shell-command-line${hasBody ? " has-body" : ""}`}>
        <span>$</span>
        <code>{command}</code>
      </div>
      {result && !success && (
        <div className="shell-exit failed">
          {t(`exit ${exitCode ?? "unknown"}`, `退出码 ${exitCode ?? "未知"}`)}
        </div>
      )}
      {stdout && (diffOutput ? <DiffView source={stdout} /> : <CollapsibleOutput source={stdout} />)}
      {stderr && <CollapsibleOutput source={stderr} className="shell-output stderr" />}
      {!result && output && <CollapsibleOutput source={output} />}
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
