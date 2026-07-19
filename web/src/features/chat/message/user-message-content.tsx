import { BookOpen, Target, X } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { Button } from "../../../shared/ui/button/button";
import { parseComposerAtoms, type ComposerAtomSegment } from "../composer/composer-atom-token";
import { MarkdownRenderer } from "../markdown-renderer";
import { useI18n } from "../../i18n/use-i18n";
import { UserMessageAtom } from "./user-message-atom";

type MessageAtom = Exclude<ComposerAtomSegment, { type: "text" }>;

type AtomEntry = {
  atom: MessageAtom;
  preview: string | null;
};

/**
 * 渲染支持输入原子还原的用户消息正文。
 *
 * @param props 用户消息原始协议文本
 * @returns 保留 Markdown 且可展开 Skill 或 Goal 的正文
 */
export function UserMessageContent({ content }: { content: string }) {
  const { t } = useI18n();
  const [expandedIndex, setExpandedIndex] = useState<number | null>(null);
  const presentation = buildAtomizedMessage(content);
  const expanded = expandedIndex === null ? null : presentation.atoms[expandedIndex] ?? null;

  useEffect(() => setExpandedIndex(null), [content]);

  const inlineAtoms: ReactNode[] = presentation.atoms.map((entry, index) => (
    <UserMessageAtom
      key={`${entry.atom.type}-${index}`}
      atom={entry.atom}
      expanded={expandedIndex === index}
      onActivate={entry.preview
        ? () => setExpandedIndex((current) => current === index ? null : index)
        : undefined}
    />
  ));

  return (
    <div className="user-message-content">
      <MarkdownRenderer source={presentation.source} inlineAtoms={inlineAtoms} />
      {expanded?.preview && (
        <div className={`user-atom-preview user-${expanded.atom.type}-preview`}>
          <div className="user-atom-preview-header">
            {expanded.atom.type === "skill" ? <BookOpen size={13} /> : <Target size={13} />}
            <span>{expanded.atom.type === "skill" ? `/${expanded.atom.name}` : t("Goal details", "目标详情")}</span>
            <Button
              className="user-atom-preview-close"
              onClick={() => setExpandedIndex(null)}
              aria-label={t("Close preview", "关闭预览")}
              title={t("Close preview", "关闭预览")}
            >
              <X size={12} />
            </Button>
          </div>
          <div className="user-atom-preview-body">
            {expanded.atom.type === "skill"
              ? <MarkdownRenderer source={expanded.preview} />
              : <p>{expanded.preview}</p>}
          </div>
        </div>
      )}
    </div>
  );
}

/**
 * 将消息协议转换为 Markdown 占位符和可交互原子列表。
 *
 * @param content 用户消息原文
 * @returns Markdown 源文本和原子预览信息
 */
function buildAtomizedMessage(content: string): { source: string; atoms: AtomEntry[] } {
  const segments = parseComposerAtoms(content);
  const atoms: AtomEntry[] = [];
  let source = "";
  for (let index = 0; index < segments.length; index += 1) {
    const segment = segments[index];
    if (segment.type === "text") {
      source += segment.value;
      continue;
    }
    const preview = segment.type === "skill"
      ? segment.content?.trim() || null
      : segment.type === "goal"
        ? goalObjective(segments, index)
        : null;
    source += `\`sai-atom-${atoms.length}\``;
    atoms.push({ atom: segment, preview });
  }
  return { source, atoms };
}

/**
 * 提取 `/goal` 原子之后的可见目标文本。
 *
 * @param segments 已解析消息片段
 * @param goalIndex Goal 原子位置
 * @returns 目标详情；没有内容时返回空
 */
function goalObjective(segments: ComposerAtomSegment[], goalIndex: number): string | null {
  const value = segments
    .slice(goalIndex + 1)
    .map((segment) => segment.type === "skill" ? `/${segment.name}` : segment.value)
    .join("")
    .trim();
  return value || null;
}
