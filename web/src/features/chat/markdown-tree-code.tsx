import "./markdown-tree-code.css";

/**
 * 判断文本是否包含目录树分支，普通代码仍交给语法高亮器。
 * @param source 代码块原文
 * @returns 是否需要连续的目录连接线
 */
export function isTreeCode(source: string): boolean {
  return /^(?:[ │]*)(?:├|└)─/mu.test(source);
}

/**
 * 按实际行高绘制树形连接符，保留原字符以支持选择和复制。
 * @param source 原文；showLineNumbers 是否显示行号
 * @returns 文字和连接线共用同一行高的代码块
 */
export function MarkdownTreeCode({ source, showLineNumbers }: { source: string; showLineNumbers: boolean }) {
  return <code className="markdown-tree-code">{source.split("\n").map((line, index, lines) => {
    const prefix = /^[ │├└─]*/u.exec(line)?.[0] ?? "";
    return <span className="markdown-tree-line" key={index}>
      {showLineNumbers && <span className="syntax-line-number" aria-hidden>{index + 1}</span>}
      <span>{Array.from(prefix).map((char, position) => char === " " ? char :
        <span className="tree-connector" data-glyph={char} key={position}>{char}</span>)}{line.slice(prefix.length)}{index < lines.length - 1 ? "\n" : ""}</span>
    </span>;
  })}</code>;
}
