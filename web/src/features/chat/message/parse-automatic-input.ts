export type AutomaticInputField = {
  label: string;
  value: string;
};

export type AutomaticInputNotice = {
  fields: AutomaticInputField[];
  leftover: string;
};

export type AutomaticInputModel = {
  title: string;
  notices: AutomaticInputNotice[];
};

const FIELD_LINE = /^([^：:]{1,20})[：:]\s*(.*)$/;

/**
 * 把自动续作回执拆成标题和「标签：值」行。
 *
 * 后端用换行分隔字段，Markdown 会把单换行收成空格，
 * 长命令就会把「日志」从中间切开。按行解析后各自换行。
 *
 * @param content 自动输入展示文本
 * @returns 标题与若干回执块
 */
export function parseAutomaticInput(content: string): AutomaticInputModel {
  const blocks = content.trim().split(/\n\s*\n/).filter(Boolean);
  if (blocks.length === 0) {
    return { title: "", notices: [] };
  }
  const title = blocks[0];
  const notices = blocks.slice(1).map(parseNotice).filter((notice) => (
    notice.fields.length > 0 || notice.leftover.length > 0
  ));
  return { title, notices };
}

/**
 * 解析一块回执里的字段行。
 *
 * @param block 以空行分隔的一段正文
 * @returns 字段与无法识别的剩余文本
 */
function parseNotice(block: string): AutomaticInputNotice {
  const fields: AutomaticInputField[] = [];
  const leftover: string[] = [];
  for (const line of block.split("\n")) {
    const match = line.match(FIELD_LINE);
    if (match) {
      fields.push({ label: match[1].trim(), value: match[2].trim() });
      continue;
    }
    if (line.trim()) leftover.push(line);
  }
  return { fields, leftover: leftover.join("\n") };
}
