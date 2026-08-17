import { CollapsibleOutput } from "./collapsible-output";
import { DiffView } from "./diff-view";
import { ToolPanel } from "./layout/tool-panel";
import { ShellCommandLine } from "./shell-command-line";
import { lenientStringField, parseJsonRecord, stringField } from "./tool-data";
import { looksLikeJsonFragment } from "./tool-display-summary";
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
  const command = stringField(args, "command")
    || lenientStringField(argumentsText, "command")
    || (looksLikeJsonFragment(argumentsText) ? "" : argumentsText);
  // 前台超时提升为后台任务：不是失败，展示去向与已产生的部分输出
  const background = result?.mode === "background";
  const stdout = stringField(result, "stdout") || stringField(result, "partial_stdout");
  const stderr = stringField(result, "stderr") || stringField(result, "partial_stderr");
  const exitCode = typeof result?.exit_code === "number" ? result.exit_code : null;
  const success = background || (typeof result?.success === "boolean" ? result.success : exitCode === 0);
  const diffOutput = isDiffCommand(command, stdout);
  const hasBody = Boolean(stdout || stderr || background || (!result && output));
  return (
    <ToolPanel className="shell-tool-view">
      <ShellCommandLine command={command} hasBody={hasBody} />
      {background && (
        <div className="shell-background-note">
          {t("Promoted to background task", "已转入后台任务")}
          {stringField(result, "task_id") && <code>{stringField(result, "task_id")}</code>}
        </div>
      )}
      {result && !success && (
        <div className="shell-exit failed">
          {t(`exit ${exitCode ?? "unknown"}`, `退出码 ${exitCode ?? "未知"}`)}
        </div>
      )}
      {stdout && (diffOutput ? <DiffView source={stdout} /> : <CollapsibleOutput source={stdout} />)}
      {stderr && <CollapsibleOutput source={stderr} className="shell-output stderr" />}
      {!result && output && <CollapsibleOutput source={output} />}
    </ToolPanel>
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
