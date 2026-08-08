import { BookMarked, ChevronDown, ChevronUp, FileText, Loader2 } from "lucide-react";
import { useQuery } from "@tanstack/react-query";
import { useEffect, useMemo, useRef, useState } from "react";
import { api } from "../../../api/client";
import type { RunMode, RunModelSelection, SessionContextPromptSection } from "../../../api/contracts";
import { HoverRevealButton } from "../../../shared/ui/hover-reveal-button/hover-reveal-button";
import { MarkdownRenderer } from "../markdown-renderer";
import { useI18n } from "../../i18n/use-i18n";
import { formatContextPromptMarkdown } from "./format-context-prompt-markdown";
import { useCollapseAnchor } from "./use-collapse-anchor";
import "./context-prompt-banner.css";

type ContextPromptBannerProps = {
  sessionId: string;
  agentId?: string | null;
  mode: RunMode;
  selection: RunModelSelection | null;
};

/**
 * 在对话首条消息前展示可展开的系统提示词、指令文件与工具描述。
 *
 * @param props 会话与 Agent 标识
 * @returns 可折叠的上下文提示词横幅
 */
type ContextPromptMeta = {
  source?: string;
  has_instruction_files?: boolean;
  has_skills?: boolean;
  has_memory?: boolean;
  has_dynamic?: boolean;
  has_tools?: boolean;
  tool_count?: number;
  sections?: SessionContextPromptSection[];
};

type ContextPromptTag = {
  id: string;
  label: string;
};

/**
 * 组装上下文横幅标签，避免前端摘要标签与后端 sections 语义重复。
 *
 * @param data 会话上下文提示词元数据
 * @param t 双语翻译函数
 * @returns 去重后的标签列表
 */
export function buildContextPromptTags(
  data: ContextPromptMeta | undefined,
  t: (en: string, zh: string) => string
): ContextPromptTag[] {
  if (!data) return [];
  const tags: ContextPromptTag[] = [];
  /** 添加标签并按稳定标识去重。 */
  const pushTag = (id: string, label: string, prepend = false) => {
    if (!id.trim() || !label.trim() || tags.some((tag) => tag.id === id)) return;
    if (prepend) tags.unshift({ id, label });
    else tags.push({ id, label });
  };
  // 1. 后端 sections 已按请求语言本地化，并提供稳定导航标识
  for (const section of data.sections ?? []) {
    pushTag(section.id, section.label);
  }
  // 2. 仅补充 sections 未覆盖的摘要信息
  if (data.has_instruction_files && !tags.some((tag) => tag.label === "AGENT.md")) {
    pushTag("baseline", t("AGENT.md", "AGENT.md"), true);
  }
  if (data.has_skills && !tags.some((tag) => isSkillTag(tag.label))) {
    pushTag("baseline", t("Skills", "技能目录"), true);
  }
  if (data.has_memory && !tags.some((tag) => isMemoryTag(tag.label))) {
    pushTag("memory", t("Memory", "关联记忆"));
  }
  if (data.has_dynamic && !tags.some((tag) => isDynamicTag(tag.label))) {
    pushTag("runtime", t("Dynamic", "动态段"));
  }
  // 3. 工具定义已由 sections 提供时不再追加“工具 (n)”
  if (data.has_tools && !tags.some((tag) => isToolTag(tag.label))) {
    const count = data.tool_count ?? 0;
    pushTag("tools", count > 0 ? t(`Tools (${count})`, `工具 (${count})`) : t("Tools", "工具"));
  }
  if (data.source === "session_baseline" && !tags.some((tag) => isBaselineTag(tag.label))) {
    pushTag("baseline", t("Session baseline", "会话 baseline"), true);
  } else if (data.source === "live" && !tags.some((tag) => isLivePreviewTag(tag.label))) {
    pushTag("baseline", t("Live preview", "实时预览"), true);
  }
  return tags.slice(0, 10);
}

/**
 * 判断标签是否表示工具定义。
 *
 * @param tag 标签文本
 * @returns 命中工具语义时返回 true
 */
function isToolTag(tag: string): boolean {
  return /工具|tool/i.test(tag);
}

/**
 * 判断标签是否表示技能目录。
 *
 * @param tag 标签文本
 * @returns 命中技能语义时返回 true
 */
function isSkillTag(tag: string): boolean {
  return /技能|skill/i.test(tag);
}

/**
 * 判断标签是否表示关联记忆。
 *
 * @param tag 标签文本
 * @returns 命中记忆语义时返回 true
 */
function isMemoryTag(tag: string): boolean {
  return /记忆|memory/i.test(tag);
}

/**
 * 判断标签是否表示动态系统段。
 *
 * @param tag 标签文本
 * @returns 命中动态段语义时返回 true
 */
function isDynamicTag(tag: string): boolean {
  return /动态|dynamic|模式提醒|mode reminder|当前模型|selected model|运行时|runtime|goal|压缩摘要|compaction/i.test(tag);
}

/**
 * 判断标签是否表示会话 baseline。
 *
 * @param tag 标签文本
 * @returns 命中 baseline 语义时返回 true
 */
function isBaselineTag(tag: string): boolean {
  return /baseline/i.test(tag);
}

/**
 * 判断标签是否表示实时预览。
 *
 * @param tag 标签文本
 * @returns 命中实时预览语义时返回 true
 */
function isLivePreviewTag(tag: string): boolean {
  return /实时预览|live preview/i.test(tag);
}

export function ContextPromptBanner({
  sessionId,
  agentId,
  mode,
  selection
}: ContextPromptBannerProps) {
  const { locale, t } = useI18n();
  const [open, setOpen] = useState(false);
  const [pendingSectionId, setPendingSectionId] = useState<string | null>(null);
  const markdownRef = useRef<HTMLDivElement | null>(null);
  const anchor = useCollapseAnchor(markdownRef, open);
  const query = useQuery({
    queryKey: [
      "session-context-prompt",
      sessionId,
      agentId ?? "",
      mode,
      selection?.providerId ?? "",
      selection?.model ?? "",
      locale
    ],
    queryFn: () => api.sessions.contextPrompt(sessionId, {
      agentId: agentId ?? undefined,
      mode,
      selection,
      locale
    }),
    enabled: Boolean(sessionId),
    staleTime: 30_000
  });

  const renderedSections = useMemo(
    () => (query.data?.sections ?? []).map((section) => ({
      ...section,
      rendered: formatContextPromptMarkdown(section.content, locale)
    })),
    [locale, query.data?.sections]
  );
  const renderedFallback = useMemo(
    () => renderedSections.length > 0 ? "" : formatContextPromptMarkdown(query.data?.content ?? "", locale),
    [locale, query.data?.content, renderedSections.length]
  );

  const meta = useMemo(
    () => buildContextPromptTags(query.data, t),
    [query.data, t]
  );

  const title = t("Loaded context", "已载入上下文");
  // token 数是这张卡最该被看到的量，放在标题右侧，不与内容标签混在一起
  const tokenCount = query.data?.token_count;
  const subtitle = query.isLoading
    ? t("Loading system prompt, tools and instruction files", "正在读取系统提示词、工具与指令文件")
    : query.error
      ? t("Failed to load context prompt", "读取上下文提示词失败")
      : t(
          "Stable system prompt, dynamic segments, memory and tools",
          "稳定系统提示、动态段、记忆与工具描述"
        );

  /**
   * 收起展开后的提示词正文。
   */
  const collapse = () => setOpen(false);

  /**
   * 展开上下文并定位到标签对应的 Markdown 标题。
   *
   * @param tag 用户点击的上下文标签
   * @returns 无返回值
   */
  const revealTag = (sectionId: string) => {
    setPendingSectionId(sectionId);
    setOpen(true);
  };

  useEffect(() => {
    if (!open || !pendingSectionId || renderedSections.length === 0) return;
    const frame = window.requestAnimationFrame(() => {
      const target = findContextSection(markdownRef.current, pendingSectionId);
      target?.scrollIntoView({ block: "start", behavior: "smooth" });
      setPendingSectionId(null);
    });
    return () => window.cancelAnimationFrame(frame);
  }, [open, pendingSectionId, renderedSections.length]);

  return (
    <section className={`context-prompt-banner${open ? " open" : ""}`} data-overview-id="context-prompt">
      <div className="context-prompt-banner-head">
        <button
          type="button"
          className="context-prompt-banner-toggle"
          onClick={() => setOpen((value) => !value)}
          aria-expanded={open}
          aria-controls="context-prompt-body"
          aria-label={title}
        >
        <span className="context-prompt-banner-icon" aria-hidden>
          {query.isLoading ? <Loader2 size={14} className="spin" /> : <BookMarked size={14} />}
        </span>
          <span className="context-prompt-banner-copy">
          <span className="context-prompt-banner-title">
            {title}
            {typeof tokenCount === "number" && (
              <span className="context-prompt-banner-tokens tnum">
                {t(`~${formatTokenCount(tokenCount)} tokens`, `约 ${formatTokenCount(tokenCount)} tokens`)}
              </span>
            )}
            </span>
            <span className="context-prompt-banner-subtitle">{subtitle}</span>
          </span>
        </button>
        {meta.length > 0 && (
          <span className="context-prompt-banner-tags" role="list" aria-label={t("Context sections", "上下文段落")}>
            {meta.map((tag) => (
              <button
                key={`${tag.id}:${tag.label}`}
                type="button"
                className="context-prompt-banner-tag"
                onClick={() => revealTag(tag.id)}
                aria-label={t(`Open ${tag.label}`, `打开${tag.label}`)}
              >
                <FileText size={11} aria-hidden />
                {tag.label}
              </button>
            ))}
          </span>
        )}
        <button
          type="button"
          className="context-prompt-banner-chevron-button"
          onClick={() => setOpen((value) => !value)}
          aria-expanded={open}
          aria-controls="context-prompt-body"
          aria-label={title}
        >
          <ChevronDown size={14} className={`context-prompt-banner-chevron${open ? " rotate" : ""}`} aria-hidden />
        </button>
      </div>
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
          {(renderedSections.length > 0 || renderedFallback) && (
            <div ref={markdownRef} className="context-prompt-banner-markdown">
              {renderedSections.map((section) => (
                <section key={section.id} data-context-section={section.id}>
                  <MarkdownRenderer source={section.rendered} />
                </section>
              ))}
              {renderedFallback && <MarkdownRenderer source={renderedFallback} />}
            </div>
          )}
          {!query.isLoading && !query.error && renderedSections.length === 0 && !renderedFallback.trim() && (
            <div className="context-prompt-banner-status">
              {t("No system prompt content", "暂无系统提示词内容")}
            </div>
          )}
          {anchor && (
            <HoverRevealButton
              className="context-prompt-banner-collapse is-reversed"
              style={{ top: `${anchor.top}px`, right: `${anchor.right}px` }}
              icon={<ChevronUp size={14} />}
              label={t("Collapse context prompt", "收起系统提示词")}
              onClick={collapse}
            />
          )}
        </div>
      )}
    </section>
  );
}

/**
 * 根据稳定分区标识定位上下文内容。
 *
 * @param root Markdown 内容根节点
 * @param sectionId 后端提供的稳定分区标识
 * @returns 匹配到的分区节点
 */
export function findContextSection(root: HTMLDivElement | null, sectionId: string): HTMLElement | null {
  if (!root) return null;
  return Array.from(root.querySelectorAll<HTMLElement>("[data-context-section]"))
    .find((section) => section.dataset.contextSection === sectionId) ?? null;
}

/**
 * 压缩 token 数的显示位数。
 *
 * 上下文动辄数万 token，完整数字读起来费力，超过一万后改用 k 表示。
 *
 * @param count 预估 token 数
 * @returns 用于展示的字符串
 */
function formatTokenCount(count: number): string {
  if (count < 10_000) return String(count);
  return `${(count / 1000).toFixed(count < 100_000 ? 1 : 0)}k`;
}
