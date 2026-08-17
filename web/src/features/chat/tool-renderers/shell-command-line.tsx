import { SyntaxHighlighter } from "../syntax-highlighter";
import "../markdown-renderer.css";

type ShellCommandLineProps = {
  command: string;
  hasBody?: boolean;
};

/**
 * 渲染带提示符的 Shell 命令行，命令走 bash 语法着色。
 *
 * @param props 命令文本与是否带下方输出
 * @returns 终端风格命令行
 */
export function ShellCommandLine({ command, hasBody = false }: ShellCommandLineProps) {
  return (
    <div className={`shell-command-line${hasBody ? " has-body" : ""}`}>
      <span className="shell-command-prompt" aria-hidden>$</span>
      <HighlightedShellCommand command={command} />
    </div>
  );
}

/**
 * 渲染可嵌入摘要行的着色命令。
 *
 * @param props 命令文本
 * @returns 等宽着色命令
 */
export function HighlightedShellCommand({ command }: { command: string }) {
  return (
    <span className="shell-command-highlight">
      <SyntaxHighlighter language="bash" source={command} />
    </span>
  );
}
