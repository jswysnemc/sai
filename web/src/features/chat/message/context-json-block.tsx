import { ChevronDown, ChevronRight } from "lucide-react";
import { useId, useState } from "react";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import type { MarkdownCodeBlockStylePreferences } from "../../markdown/markdown-style-preferences";
import { MarkdownCodeBlock } from "../markdown-code-block";

/**
 * 【上下文】【工具定义】默认折叠 JSON，展开后才创建高亮内容，保留复制操作。
 * @param props JSON 原文与当前代码块样式
 * @returns 紧凑的工具参数定义折叠区
 */
export function ContextJsonBlock({ source, style }: { source: string; style: MarkdownCodeBlockStylePreferences }) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const bodyId = useId();
  return (
    <div className="my-2 min-w-0 rounded-md border border-[var(--line)] bg-[var(--control-surface)] text-xs sm:text-sm">
      <Button
        className="flex w-full min-w-0 items-center gap-2 px-3 py-2 text-left text-[var(--ink-soft)]"
        aria-expanded={open}
        aria-controls={bodyId}
        onClick={() => setOpen((value) => !value)}
      >
        {open ? <ChevronDown size={14} aria-hidden /> : <ChevronRight size={14} aria-hidden />}
        <span className="min-w-0 flex-1">{t("Tool definition JSON", "工具定义 JSON")}</span>
        <span className="shrink-0 font-mono text-xs">{t(`${source.split("\n").length} lines`, `${source.split("\n").length} 行`)}</span>
      </Button>
      {open && <div id={bodyId} className="min-w-0 overflow-x-auto border-t border-[var(--line)]"><MarkdownCodeBlock language="json" source={source} style={style} /></div>}
    </div>
  );
}
