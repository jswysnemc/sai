import type { PromptSectionId, PromptSectionToggles } from "../../../api/contracts";
import { useI18n } from "../../i18n/use-i18n";
import type { PromptSectionOption } from "./agents-types";

type AgentPromptSectionsProps = {
  sections?: PromptSectionToggles;
  /** 后端给出的分段清单；为空时回退到内置兜底列表 */
  options?: PromptSectionOption[];
  onChange: (sections: PromptSectionToggles) => void;
};

/** 后端不可用时的兜底清单，正常情况下由 /api/agent-options 提供。 */
const FALLBACK_SECTIONS: PromptSectionOption[] = [
  {
    id: "builtin_persona",
    label_en: "Built-in persona",
    label_zh: "内置人设",
    hint_en: "Fallback persona used when this agent's own prompt is empty. Turn off for a truly blank prompt.",
    hint_zh: "本 Agent 提示词为空时的兜底人设。要配出真正空白的提示词就关掉它。"
  },
  {
    id: "user_identity",
    label_en: "User identity",
    label_zh: "用户身份",
    hint_en: "The active user profile appended as <current-user-profile>.",
    hint_zh: "以 <current-user-profile> 追加的当前用户档案。"
  },
  {
    id: "skills_catalog",
    label_en: "Skills catalog",
    label_zh: "技能目录",
    hint_en: "List of available skills, independent of whether skill tools are registered.",
    hint_zh: "可用 skills 清单，与是否注册技能工具相互独立。"
  },
  {
    id: "state_contract",
    label_en: "Runtime context",
    label_zh: "运行时上下文",
    hint_en: "Working directory, time, model, shell, current Goal and the contract for reading their state tags.",
    hint_zh: "工作目录、时间、模型、Shell、当前 Goal，以及读取这些状态标签的说明。"
  },
  {
    id: "memory_contract",
    label_en: "Memory context",
    label_zh: "记忆上下文",
    hint_en: "Memory usage rules and the relevant memory index for this turn. Only injected when memory is enabled.",
    hint_zh: "记忆使用规则与本轮相关记忆索引，仅在记忆功能启用时注入。"
  },
  {
    id: "mode_reminder",
    label_en: "Mode reminder",
    label_zh: "模式提示词",
    hint_en: "Constraints for the current run mode: YOLO, audited or plan.",
    hint_zh: "当前运行模式的约束说明：YOLO、审计或计划模式。"
  }
];

/**
 * 系统提示词内置分段的开关面板。
 *
 * 这些内容原本硬拼在提示词里，界面上看不见也关不掉，"0 提示词"因此
 * 无法表达。全部默认开启，不配置就是原有行为。
 *
 * @param props 当前开关与变更回调
 * @returns 分段开关列表
 */
export function AgentPromptSections({ sections, options, onChange }: AgentPromptSectionsProps) {
  const { t } = useI18n();
  const catalog = options?.length ? options : FALLBACK_SECTIONS;
  const current = sections ?? {};
  const enabled = (id: string) => current[id as PromptSectionId] !== false;
  const activeCount = catalog.filter((section) => enabled(section.id)).length;

  /** 切换单个分段。 */
  const toggle = (id: string, value: boolean) => {
    onChange({ ...current, [id]: value });
  };

  /** 一次开启或关闭全部分段。 */
  const toggleAll = (value: boolean) => {
    const next: PromptSectionToggles = {};
    for (const section of catalog) next[section.id as PromptSectionId] = value;
    onChange(next);
  };

  return (
    <div className="agent-prompt-sections">
      <header className="agent-prompt-sections-head">
        <span>
          {t(
            `${activeCount}/${catalog.length} built-in sections on`,
            `已启用 ${activeCount}/${catalog.length} 个内置分段`
          )}
        </span>
        <button type="button" className="settings-secondary" onClick={() => toggleAll(activeCount > 0 ? false : true)}>
          {activeCount > 0 ? t("Turn all off", "全部关闭") : t("Turn all on", "全部开启")}
        </button>
      </header>
      <div className="agent-prompt-sections-list">
        {catalog.map((section) => (
          <label key={section.id} className="agent-prompt-section-item">
            <span>
              <strong>{t(section.label_en, section.label_zh)}</strong>
              <small>{t(section.hint_en, section.hint_zh)}</small>
            </span>
            <input
              type="checkbox"
              className="switch-control"
              checked={enabled(section.id)}
              onChange={(event) => toggle(section.id, event.target.checked)}
            />
          </label>
        ))}
      </div>
    </div>
  );
}
