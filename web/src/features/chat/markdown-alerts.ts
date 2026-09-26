type MarkdownNode = {
  type: string;
  value?: string;
  children?: MarkdownNode[];
  data?: { hProperties?: Record<string, unknown> };
};

const ALERT_KINDS = ["note", "tip", "important", "warning", "caution"] as const;

/**
 * 把以 [!NOTE] 这类标记开头的引用块标成提示条。
 *
 * @returns remark 插件
 */
export function remarkGithubAlerts() {
  return (tree: MarkdownNode) => {
    walk(tree);
  };
}

function walk(node: MarkdownNode) {
  if (node.type === "blockquote") tagAlert(node);
  node.children?.forEach(walk);
}

function tagAlert(node: MarkdownNode) {
  const paragraph = node.children?.[0];
  const first = paragraph?.children?.[0];
  if (paragraph?.type !== "paragraph" || first?.type !== "text" || !first.value) return;
  const match = /^\[!(NOTE|TIP|IMPORTANT|WARNING|CAUTION)\]\s*/i.exec(first.value);
  if (!match) return;
  const kind = match[1].toLowerCase();
  if (!ALERT_KINDS.includes(kind as (typeof ALERT_KINDS)[number])) return;
  first.value = first.value.slice(match[0].length);
  if (!first.value) {
    paragraph.children?.shift();
    if (paragraph.children?.[0]?.type === "break") paragraph.children.shift();
  }
  node.data ??= {};
  node.data.hProperties = {
    className: ["markdown-alert", `markdown-alert-${kind}`],
    "data-alert": kind
  };
}
