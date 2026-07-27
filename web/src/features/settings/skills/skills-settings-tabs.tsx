import { BookOpen, SlidersHorizontal } from "lucide-react";
import { useRef, type KeyboardEvent } from "react";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";

export type SkillsSettingsView = "library" | "behavior";

type SkillsSettingsTabsProps = {
  value: SkillsSettingsView;
  total: number;
  enabled: number;
  onChange: (view: SkillsSettingsView) => void;
};

/**
 * 渲染 Skills 页面主视图切换和库状态摘要。
 *
 * @param props 当前视图、Skill 数量及切换回调
 * @returns 技能库与运行策略页签
 */
export function SkillsSettingsTabs({ value, total, enabled, onChange }: SkillsSettingsTabsProps) {
  const { t } = useI18n();
  const tabsRef = useRef<HTMLDivElement>(null);

  /**
   * 使用方向键切换并聚焦相邻页签。
   *
   * @param event 页签容器键盘事件
   * @returns 无返回值
   */
  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
    event.preventDefault();
    const next = value === "library" ? "behavior" : "library";
    onChange(next);
    requestAnimationFrame(() => tabsRef.current?.querySelector<HTMLElement>(`[aria-controls="skills-${next === "library" ? "library" : "behavior"}-panel"]`)?.focus());
  };

  return (
    <div className="skills-settings-toolbar">
      <div ref={tabsRef} className="skills-view-tabs" role="tablist" aria-label={t("Skills settings views", "Skills 设置视图")} onKeyDown={handleKeyDown}>
        <Button
          className={value === "library" ? "skills-view-tab active" : "skills-view-tab"}
          role="tab"
          aria-selected={value === "library"}
          aria-controls="skills-library-panel"
          tabIndex={value === "library" ? 0 : -1}
          onClick={() => onChange("library")}
        >
          <BookOpen size={14} aria-hidden="true" />
          {t("Library", "技能库")}
          <span>{total}</span>
        </Button>
        <Button
          className={value === "behavior" ? "skills-view-tab active" : "skills-view-tab"}
          role="tab"
          aria-selected={value === "behavior"}
          aria-controls="skills-behavior-panel"
          tabIndex={value === "behavior" ? 0 : -1}
          onClick={() => onChange("behavior")}
        >
          <SlidersHorizontal size={14} aria-hidden="true" />
          {t("Runtime policy", "运行策略")}
        </Button>
      </div>
      <span className="skills-library-summary">
        {t(`${enabled} of ${total} enabled`, `已启用 ${enabled}/${total}`)}
      </span>
    </div>
  );
}
