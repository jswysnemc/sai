import { BookOpen } from "lucide-react";
import { forwardRef, useEffect, useMemo, useRef, useState } from "react";
import { api } from "../../../api/client";
import { useI18n } from "../../i18n/use-i18n";

export type SkillOption = {
  name: string;
  description: string;
};

type SkillMentionPopoverProps = {
  open: boolean;
  query: string;
  activeIndex: number;
  onActiveIndexChange: (index: number) => void;
  onSelect: (name: string) => void;
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
    skill.name.toLowerCase().includes(keyword)
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
  function SkillMentionPopover({ open, query, activeIndex, onActiveIndexChange, onSelect }, ref) {
    const { t } = useI18n();
    const [skills, setSkills] = useState<SkillOption[]>([]);
    const [loading, setLoading] = useState(false);
    const listRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
      if (!open) return;
      let cancelled = false;
      setLoading(true);
      api.skills
        .list()
        .then((response) => {
          if (!cancelled) setSkills(response.skills);
        })
        .catch(() => {
          if (!cancelled) setSkills([]);
        })
        .finally(() => {
          if (!cancelled) setLoading(false);
        });
      return () => {
        cancelled = true;
      };
    }, [open]);

    const filtered = useMemo(() => filterSkills(skills, query), [skills, query]);

    useEffect(() => {
      if (!open) return;
      onActiveIndexChange(Math.min(activeIndex, Math.max(filtered.length - 1, 0)));
    }, [activeIndex, filtered.length, onActiveIndexChange, open]);

    useEffect(() => {
      const item = listRef.current?.children[activeIndex] as HTMLElement | undefined;
      item?.scrollIntoView({ block: "nearest" });
    }, [activeIndex, filtered.length]);

    if (!open) return null;
    return (
      <div className="file-mention-popover skill-mention-popover" role="listbox" aria-label={t("Choose a Skill", "选择 Skill")} ref={ref}>
        <div className="file-mention-filter skill-mention-title">{t("Choose a Skill · ↑↓ navigate · Enter select · Esc close", "选择 Skill · ↑↓ 导航 · Enter 确认 · Esc 关闭")}</div>
        <div className="file-mention-list" ref={listRef}>
          {filtered.map((skill, index) => (
            <button
              type="button"
              role="option"
              aria-selected={index === activeIndex}
              className={index === activeIndex ? "file-mention-item active" : "file-mention-item"}
              onMouseEnter={() => onActiveIndexChange(index)}
              onClick={() => onSelect(skill.name)}
              key={skill.name}
            >
              <BookOpen size={12} />
              <span className="skill-mention-name">/{skill.name}</span>
              {skill.description && <span className="skill-mention-desc">{skill.description}</span>}
            </button>
          ))}
          {filtered.length === 0 && (
            <div className="file-mention-empty">{loading ? t("Loading Skills", "正在加载 Skills") : t("No matching Skills", "没有匹配的 Skill")}</div>
          )}
        </div>
      </div>
    );
  }
);
