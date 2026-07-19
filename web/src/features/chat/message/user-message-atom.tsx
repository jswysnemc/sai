import { BookOpen, FileText, SquareTerminal, Target, type LucideIcon } from "lucide-react";
import type { ComposerAtomSegment } from "../composer/composer-atom-token";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";

type MessageAtom = Exclude<ComposerAtomSegment, { type: "text" }>;

type UserMessageAtomProps = {
  atom: MessageAtom;
  expanded?: boolean;
  onActivate?: () => void;
};

/**
 * 渲染用户气泡内的文件、Skill、Goal 或终端选区原子。
 *
 * @param props 原子数据、展开状态和激活回调
 * @returns 与输入区语义一致的紧凑胶囊
 */
export function UserMessageAtom({ atom, expanded = false, onActivate }: UserMessageAtomProps) {
  const { t } = useI18n();
  const presentation = atomPresentation(atom);
  const content = (
    <>
      <presentation.Icon size={12} aria-hidden />
      <span>{presentation.label}</span>
    </>
  );
  const className = `user-message-atom user-${atom.type}-atom`;
  const preview = atom.type === "terminal" ? terminalPreview(atom.content) : undefined;

  if (onActivate) {
    return (
      <Button
        className={className}
        aria-expanded={expanded}
        onClick={onActivate}
        title={presentation.title}
      >
        {content}
      </Button>
    );
  }
  return (
    <span
      className={className}
      title={presentation.title}
      data-preview={preview}
      aria-label={atom.type === "terminal" ? t("Terminal selection", "终端选区") : undefined}
    >
      {content}
    </span>
  );
}

/**
 * 返回不同气泡原子的图标、标签和悬停说明。
 *
 * @param atom 待展示原子
 * @returns 原子视觉信息
 */
function atomPresentation(atom: MessageAtom): { Icon: LucideIcon; label: string; title: string } {
  if (atom.type === "file") return { Icon: FileText, label: atom.path, title: atom.path };
  if (atom.type === "skill") return { Icon: BookOpen, label: `/${atom.name}`, title: `Skill: ${atom.name}` };
  if (atom.type === "goal") return { Icon: Target, label: "/goal", title: "Session goal" };
  const lines = atom.content.split(/\r?\n/u).length;
  return {
    Icon: SquareTerminal,
    label: `${atom.source || "Terminal"} · ${lines} lines`,
    title: atom.content
  };
}

/**
 * 限制终端选区悬停预览长度。
 *
 * @param content 原始终端选区
 * @returns 有界预览文本
 */
function terminalPreview(content: string): string {
  const limit = 1_200;
  return content.length > limit ? `${content.slice(0, limit)}\n...` : content;
}
