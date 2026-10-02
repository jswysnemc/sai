import { BookOpen, Target } from "../../../shared/ui/icons";
import { forwardRef, useEffect, useMemo, useRef, useState, type RefObject } from "react";
import { useQuery } from "@tanstack/react-query";
import { templateApi, templateKey, type TemplateScope } from "../../prompt-templates/template-client";
import { Button } from "../../../shared/ui/button/button";
import { api } from "../../../api/client";
import { useI18n } from "../../i18n/use-i18n";
import { MentionPopoverPortal } from "./mention-popover-portal";

export type SkillOption = {
  name: string;
  description: string;
  kind?: "skill" | "command" | "template";
  content?: string;
  keyword?: string;
};

type SkillMentionPopoverProps = {
  open: boolean;
  scope?: TemplateScope;
  allowPlan?: boolean;
  anchorRef: RefObject<HTMLElement | null>;
  query: string;
  activeIndex: number;
  onActiveIndexChange: (index: number) => void;
  onSelect: (name: string) => void;
  onOptionsChange: (options: SkillOption[]) => void;
};

/**
 * 按名称与描述模糊过滤 skill 列表。
 *
 * @param skills skill 选项
 * @param query 过滤关键词
 * @returns 匹配项
 */
export function filterSkills(skills: SkillOption[], query: string): SkillOption[] {
  const keyword = query.trim().toLowerCase();
  if (!keyword) return skills;
  return skills.filter((skill) =>
    (skill.keyword ?? skill.name).toLowerCase().includes(keyword)
    || skill.description.toLowerCase().includes(keyword)
  );
}

/**
 * 渲染输入框上方的 skill 选择浮层。
 *
 * 键盘导航由输入区处理，这里只负责列表展示与点击选择。
 *
 * @param props 打开状态、过滤词、高亮项与选中回调
 * @returns skill 浮层；关闭时返回 null
 */
export const SkillMentionPopover = forwardRef<HTMLDivElement, SkillMentionPopoverProps>(
  function SkillMentionPopover({ scope = "chat", allowPlan = false, open, anchorRef, query, activeIndex, onActiveIndexChange, onSelect, onOptionsChange }, ref) {
    const { t } = useI18n();
    const [skills, setSkills] = useState<SkillOption[]>([]);
    const templates = useQuery({ queryKey: templateKey(scope), queryFn: () => templateApi.list(scope), enabled: open, staleTime: 30_000 });
    const [loading, setLoading] = useState(false);
    const listRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
      if (!open || scope === "image") return;
      let cancelled = false;
      setLoading(true);
      api.skills
        .list()
        .then((response) => {
          if (cancelled) return;
          const options: SkillOption[] = [
            {
              name: "goal",
              description: t("Use the remaining input as the session goal", "将后续输入设为会话目标"),
              kind: "command"
            },
            ...response.skills.map((skill) => ({ ...skill, kind: "skill" as const }))
          ];
          setSkills(options);
        })
        .catch(() => {
          if (cancelled) return;
          const options: SkillOption[] = [{
            name: "goal",
            description: t("Use the remaining input as the session goal", "将后续输入设为会话目标"),
            kind: "command"
          }];
          setSkills(options);
        })
        .finally(() => {
          if (!cancelled) setLoading(false);
        });
      return () => {
        cancelled = true;
      };
    }, [scope, open, t]);

    const options = useMemo<SkillOption[]>(() => [
      ...(scope === "chat" && allowPlan ? [{ name: "plan", description: t("Enter planning; review before execution", "进入规划，审阅后执行"), kind: "command" as const }] : []),
      ...(scope === "chat" ? skills : []),
      ...(templates.data ?? []).map((item) => ({
        name: `template:${item.name}`, keyword: item.name, content: item.content,
        description: `${t("Template", "模板")} · ${item.content.split("\n")[0]}`,
        kind: "template" as const
      }))
    ], [scope, allowPlan, skills, templates.data, t]);
    useEffect(() => { onOptionsChange(options); }, [options, onOptionsChange]);
    const filtered = useMemo(() => filterSkills(options, query), [options, query]);

    useEffect(() => {
      if (!open) return;
      onActiveIndexChange(Math.min(activeIndex, Math.max(filtered.length - 1, 0)));
    }, [activeIndex, filtered.length, onActiveIndexChange, open]);

    useEffect(() => {
      const item = listRef.current?.children[activeIndex] as HTMLElement | undefined;
      item?.scrollIntoView({ block: "nearest" });
    }, [activeIndex, filtered.length]);

    return (
      <MentionPopoverPortal
        ref={ref}
        open={open}
        anchorRef={anchorRef}
        className="skill-mention-popover"
        ariaLabel={t("Commands and templates", "命令与模板")}
      >
        <div className="file-mention-filter skill-mention-title">{t("↑↓ navigate · Enter / Tab insert · Esc close", "↑↓ 导航 · Enter / Tab 插入 · Esc 关闭")}</div>
        {templates.isError && <div role="alert" className="file-mention-empty">{t("Unable to load templates", "无法加载模板")}</div>}
        <div className="file-mention-list" ref={listRef}>
          {filtered.map((skill, index) => (
            <Button variant="ghost"
              type="button"
              role="option"
              aria-selected={index === activeIndex}
              className={index === activeIndex ? "file-mention-item active" : "file-mention-item"}
              onMouseDown={(event) => event.preventDefault()}
              onMouseEnter={() => onActiveIndexChange(index)}
              onClick={() => onSelect(skill.name)}
              key={skill.name}
            >
              {skill.kind === "command" ? <Target size={12} /> : <BookOpen size={12} />}
              <span className="skill-mention-name">/{skill.keyword ?? skill.name}</span>
              {skill.description && <span className="skill-mention-desc">{skill.description}</span>}
            </Button>
          ))}
          {filtered.length === 0 && (
            <div className="file-mention-empty">{loading || templates.isLoading ? t("Loading…", "正在加载…") : t("No matches", "没有匹配项")}</div>
          )}
        </div>
      </MentionPopoverPortal>
    );
  }
);
