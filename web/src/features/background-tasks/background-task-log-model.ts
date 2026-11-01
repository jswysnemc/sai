import type { BackgroundTaskOutput } from "../../api/contracts";

export type BackgroundTaskStream = "stdout" | "stderr" | "all";
export type BackgroundTaskLogLine = { stream: "stdout" | "stderr"; number: number; text: string };

/**
 * 清除日志中的终端转义与不可见控制字符，保留普通文本和制表符。
 * @param value 原始日志文本
 * @returns 可直接展示的日志文本
 */
export function cleanBackgroundTaskLog(value: string): string {
  return value
    .replace(/\x1b(?:\][^\x07]*(?:\x07|\x1b\\)|\[[0-?]*[ -/]*[@-~]|[PX^_][\s\S]*?\x1b\\|[ -/]*[@-~])/g, "")
    .replace(/[\x00-\x08\x0b\x0c\x0e-\x1f\x7f]/g, "");
}

/**
 * 获取指定日志通道的缓冲行，全部模式按通道分组，不虚构输出时间顺序。
 * @param output 接口输出快照
 * @param stream 所选通道
 * @returns 带通道和缓冲区行号的文本行
 */
export function backgroundTaskLogLines(output: BackgroundTaskOutput | undefined, stream: BackgroundTaskStream): BackgroundTaskLogLine[] {
  if (!output) return [];
  return (["stdout", "stderr"] as const).flatMap((source) => {
    if (stream !== "all" && stream !== source) return [];
    const value = cleanBackgroundTaskLog(output[source] ?? "").replace(/\r\n/g, "\n").replace(/\r/g, "\n");
    if (!value) return [];
    const lines = value.split("\n");
    if (lines.at(-1) === "") lines.pop();
    return lines.map((text, index) => ({ stream: source, number: index + 1, text }));
  });
}

/**
 * 判断当前通道是否只保留了日志尾部。
 * @param output 接口快照
 * @param stream 所选通道
 * @returns 当前展示内容是否截断
 */
export function backgroundTaskLogTruncated(output: BackgroundTaskOutput | undefined, stream: BackgroundTaskStream): boolean {
  return Boolean(output && ((stream !== "stderr" && output.stdout_truncated) || (stream !== "stdout" && output.stderr_truncated)));
}
