import { BookMarked, ChevronDown, FileText, Loader2 } from "lucide-react";
import { useQuery } from "@tanstack/react-query";
import { useMemo, useState } from "react";
import { api } from "../../../api/client";
import { MarkdownRenderer } from "../markdown-renderer";
import { useI18n } from "../../i18n/use-i18n";
import { formatContextPromptMarkdown } from "./format-context-prompt-markdown";
import "./context-prompt-banner.css";

type ContextPromptBannerProps = {
  sessionId: string;
  agentId?: string | null;
};

/**
 * 在对话首条消息前展示可展开的系统提示词、指令文件与工具描述。
 *
 * @param props 会话与 Agent 标识
 * @returns 可折叠的上下文提示词横幅
 */
export function ContextPromptBanner({ sessionId, agentId }: ContextPromptBannerProps) {
  const { locale, t } = useI18n();
  const [open, setOpen] = useState(false);
  const query = useQuery({
    queryKey: ["session-context-prompt", sessionId, agentId ?? "", locale],
    queryFn: () => api.sessions.contextPrompt(sessionId, agentId ?? undefined, locale),
    enabled: Boolean(sessionId),
    staleTime: 30_000
  });

  const rendered = useMemo(
    () => formatContextPromptMarkdown(query.data?.content ?? "", locale),
    [locale, query.data?.content]
  );

  const meta = useMemo(() => {
    const tags: string[] = [];
    if (query.data?.has_instruction_files) {
      tags.push(t("AGENT.md", "AGENT.md"));
    }
    if (query.data?.has_skills) {
      tags.push(t("Skills", "技能目录"));
    }
    if (query.data?.has_memory) {
      tags.push(t("Memory", "关联记忆"));
    }
    if (query.data?.has_dynamic) {
      tags.push(t("Dynamic", "动态段"));
    }
    if (query.data?.has_tools) {
      const count = query.data.tool_count ?? 0;
      tags.push(count > 0 ? t(`Tools (${count})`, `工具 (${count})`) : t("Tools", "工具"));
    }
    if (query.data?.source === "session_baseline") {
      tags.push(t("Session baseline", "会话 baseline"));
    } else if (query.data?.source === "live") {
      tags.push(t("Live preview", "实时预览"));
    }
    // 后端 sections 已按请求语言本地化，作为补充标签（去重）
    for (const section of query.data?.sections ?? []) {
      if (!tags.includes(section)) tags.push(section);
    }
    return tags.slice(0, 10);
  }, [query.data, t]);

  const title = t("Loaded context", "已载入上下文");
  const subtitle = query.isLoading
    ? t("Loading system prompt, tools and instruction files", "正在读取系统提示词、工具与指令文件")
    : query.error
      ? t("Failed to load context prompt", "读取上下文提示词失败")
      : t(
          "Stable system prompt, dynamic segments, memory and tools",
          "稳定系统提示、动态段、记忆与工具描述"
        );

  return (
    <section className={`context-prompt-banner${open ? " open" : ""}`} data-overview-id="context-prompt">
      <button
        type="button"
        className="context-prompt-banner-head"
        onClick={() => setOpen((value) => !value)}
        aria-expanded={open}
        aria-controls="context-prompt-body"
      >
        <span className="context-prompt-banner-icon" aria-hidden>
          {query.isLoading ? <Loader2 size={14} className="spin" /> : <BookMarked size={14} />}
        </span>
        <span className="context-prompt-banner-copy">
          <span className="context-prompt-banner-title">{title}</span>
          <span className="context-prompt-banner-subtitle">{subtitle}</span>
          {meta.length > 0 && (
            <span className="context-prompt-banner-tags">
              {meta.map((tag) => (
                <span key={tag} className="context-prompt-banner-tag">
                  <FileText size={11} aria-hidden />
                  {tag}
                </span>
              ))}
              {typeof query.data?.char_count === "number" && (
                <span className="context-prompt-banner-tag muted">
                  {t(`${query.data.char_count} chars`, `${query.data.char_count} 字符`)}
                </span>
              )}
            </span>
          )}
        </span>
        <ChevronDown size={14} className={`context-prompt-banner-chevron${open ? " rotate" : ""}`} aria-hidden />
      </button>
      {open && (
        <div id="context-prompt-body" className="context-prompt-banner-body">
          {query.isLoading && (
            <div className="context-prompt-banner-status">
              {t("Loading…", "加载中…")}
            </div>
          )}
          {query.error && (
            <div className="context-prompt-banner-status error">
              {query.error instanceof Error ? query.error.message : String(query.error)}
            </div>
          )}
          {rendered && (
            <div className="context-prompt-banner-markdown">
              <MarkdownRenderer source={rendered} />
            </div>
          )}
          {!query.isLoading && !query.error && !rendered.trim() && (
            <div className="context-prompt-banner-status">
              {t("No system prompt content", "暂无系统提示词内容")}
            </div>
          )}
        </div>
      )}
    </section>
  );
}
