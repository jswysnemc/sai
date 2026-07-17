import { BrainCircuit, Check, ChevronDown, ChevronRight, Search } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import type { RunModelSelection, ThinkingLevel } from "../../api/contracts";
import { ModelIcon } from "../../shared/ui/model-icon";
import type { ChatModelChoice } from "./chat-model-options";
import { THINKING_OPTIONS, thinkingLevelLabel } from "./model-thinking-options";
import { useI18n } from "../i18n/use-i18n";

type ModelThinkingSelectorProps = {
  choices: ChatModelChoice[];
  selection: ChatModelChoice | null;
  thinkingLevel: ThinkingLevel;
  loading: boolean;
  disabled: boolean;
  onModelSelect: (selection: RunModelSelection) => void;
  onThinkingLevelChange: (level: ThinkingLevel) => void;
};

type SelectorSection = "model" | "thinking";

/**
 * 渲染统一的模型与推理强度二级选择菜单。
 *
 * @param props 模型选项、当前选择、禁用状态和更新回调
 * @returns 紧凑触发器与二级选择菜单
 */
export function ModelThinkingSelector(props: ModelThinkingSelectorProps) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const [section, setSection] = useState<SelectorSection>("model");
  const [query, setQuery] = useState("");
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState({ left: 12, bottom: 56 });
  const normalizedQuery = query.trim().toLocaleLowerCase();
  const filteredChoices = props.choices.filter((choice) => (
    !normalizedQuery
    || choice.model.toLocaleLowerCase().includes(normalizedQuery)
    || choice.providerName.toLocaleLowerCase().includes(normalizedQuery)
  ));
  const thinkingLabel = thinkingLevelLabel(props.thinkingLevel);

  useEffect(() => {
    if (!open) return;

    /** 在统一选择器外按下指针时关闭菜单。 */
    const handlePointerDown = (event: PointerEvent) => {
      const target = event.target as Node;
      if (!triggerRef.current?.contains(target) && !menuRef.current?.contains(target)) setOpen(false);
    };

    document.addEventListener("pointerdown", handlePointerDown);
    return () => document.removeEventListener("pointerdown", handlePointerDown);
  }, [open]);

  useEffect(() => {
    if (!open) return;

    /** 根据触发器位置计算向上展开菜单的固定坐标。 */
    const updatePosition = () => {
      const rect = triggerRef.current?.getBoundingClientRect();
      if (!rect) return;
      const padding = 12;
      const menuWidth = Math.min(590, window.innerWidth - padding * 2);
      const preferredLeft = rect.right - 220;
      const left = Math.max(padding, Math.min(preferredLeft, window.innerWidth - menuWidth - padding));
      setPosition({ left, bottom: window.innerHeight - rect.top + 8 });
    };

    updatePosition();
    window.addEventListener("resize", updatePosition);
    window.addEventListener("scroll", updatePosition, true);
    return () => {
      window.removeEventListener("resize", updatePosition);
      window.removeEventListener("scroll", updatePosition, true);
    };
  }, [open]);

  /** 打开或关闭菜单并重置临时搜索状态。 */
  const toggleMenu = () => {
    setOpen((current) => {
      if (!current) {
        setSection("model");
        setQuery("");
      }
      return !current;
    });
  };

  /**
   * 选择模型并关闭菜单。
   *
   * @param choice 用户选择的模型
   */
  const selectModel = (choice: ChatModelChoice) => {
    props.onModelSelect({ providerId: choice.providerId, model: choice.model });
    setOpen(false);
  };

  /**
   * 选择推理强度并关闭菜单。
   *
   * @param level 用户选择的推理强度
   */
  const selectThinkingLevel = (level: ThinkingLevel) => {
    props.onThinkingLevelChange(level);
    setOpen(false);
  };

  return (
    <div className="model-thinking-selector">
      <button
        ref={triggerRef}
        type="button"
        className="model-thinking-trigger"
        onClick={toggleMenu}
        disabled={props.disabled || props.loading}
        aria-haspopup="dialog"
        aria-expanded={open}
        aria-label={t(`${props.selection?.model ?? "No model configured"}, reasoning effort ${thinkingLabel}`, `${props.selection?.model ?? "未配置模型"}，推理强度 ${thinkingLabel}`)}
      >
        {props.selection?.model ? <ModelIcon model={props.selection.model} size={14} /> : null}
        <span className="model-thinking-model">{props.loading ? t("Loading models", "读取模型") : props.selection?.model ?? t("No model configured", "未配置模型")}</span>
        <span className="model-thinking-level">{thinkingLabel}</span>
        <ChevronDown size={12} className={open ? "model-thinking-chevron open" : "model-thinking-chevron"} />
      </button>
      {open && createPortal(
        <div
          ref={menuRef}
          className="model-thinking-menu"
          role="dialog"
          aria-label={t("Model and reasoning effort", "模型与推理强度")}
          style={position}
          onKeyDown={(event) => {
            if (event.key === "Escape") {
              event.preventDefault();
              setOpen(false);
              triggerRef.current?.focus();
            }
          }}
        >
          <div className="model-thinking-sections">
            <button
              type="button"
              className={section === "model" ? "active" : ""}
              onClick={() => setSection("model")}
              onPointerEnter={() => setSection("model")}
              aria-expanded={section === "model"}
              aria-label={t(`Model ${props.selection?.model ?? "not configured"}`, `模型 ${props.selection?.model ?? "未配置"}`)}
            >
              <span><small>{t("Model", "模型")}</small><strong>{props.selection?.model ?? t("Not configured", "未配置")}</strong></span>
              <ChevronRight size={13} />
            </button>
            <button
              type="button"
              className={section === "thinking" ? "active" : ""}
              onClick={() => setSection("thinking")}
              onPointerEnter={() => setSection("thinking")}
              aria-expanded={section === "thinking"}
              aria-label={t(`Reasoning effort ${thinkingLabel}`, `推理强度 ${thinkingLabel}`)}
            >
              <span><small>{t("Reasoning effort", "推理强度")}</small><strong>{thinkingLabel}</strong></span>
              <ChevronRight size={13} />
            </button>
          </div>
          <div className="model-thinking-options">
            {section === "model" ? (
              <ModelOptions
                choices={filteredChoices}
                selection={props.selection}
                query={query}
                onQueryChange={setQuery}
                onSelect={selectModel}
              />
            ) : (
              <ThinkingOptions value={props.thinkingLevel} onSelect={selectThinkingLevel} />
            )}
          </div>
        </div>,
        document.body
      )}
    </div>
  );
}

/**
 * 渲染模型搜索和模型选项。
 *
 * @param props 已过滤模型、当前选择、搜索状态和选择回调
 * @returns 模型二级菜单
 */
function ModelOptions({ choices, selection, query, onQueryChange, onSelect }: { choices: ChatModelChoice[]; selection: ChatModelChoice | null; query: string; onQueryChange: (value: string) => void; onSelect: (choice: ChatModelChoice) => void }) {
  const { t } = useI18n();
  return (
    <>
      <label className="model-thinking-search">
        <Search size={14} />
        <input value={query} onChange={(event) => onQueryChange(event.target.value)} placeholder={t("Search models or providers", "搜索模型或供应商")} aria-label={t("Search models or providers", "搜索模型或供应商")} autoFocus />
      </label>
      <div className="model-thinking-option-list" role="listbox" aria-label={t("Choose model", "选择模型")}>
        {choices.map((choice) => {
          const active = choice.providerId === selection?.providerId && choice.model === selection.model;
          return (
            <button type="button" role="option" aria-selected={active} aria-label={`${choice.model}，${choice.providerName}`} className={active ? "active" : ""} key={`${choice.providerId}-${choice.model}`} onClick={() => onSelect(choice)}>
              <span className="model-thinking-option-main"><ModelIcon model={choice.model} size={15} /><strong>{choice.model}</strong></span>
              <small>{choice.providerName}</small>
              <Check size={14} />
            </button>
          );
        })}
        {choices.length === 0 && <div className="model-thinking-empty">{t("No matching models", "没有匹配的模型")}</div>}
      </div>
    </>
  );
}

/**
 * 渲染推理强度选项。
 *
 * @param props 当前推理强度和选择回调
 * @returns 推理强度二级菜单
 */
function ThinkingOptions({ value, onSelect }: { value: ThinkingLevel; onSelect: (level: ThinkingLevel) => void }) {
  const { t } = useI18n();
  return (
    <div className="model-thinking-option-list thinking" role="listbox" aria-label={t("Choose reasoning effort", "选择推理强度")}>
      <div className="model-thinking-option-head"><BrainCircuit size={14} /><span>{t("Reasoning effort", "推理强度")}</span></div>
      {THINKING_OPTIONS.map((option) => (
        <button type="button" role="option" aria-selected={option.value === value} className={option.value === value ? "active" : ""} onClick={() => onSelect(option.value)} key={option.value}>
          <span className="model-thinking-option-copy"><strong>{option.label}</strong><small>{t(option.descriptionEn, option.descriptionZh)}</small></span>
          <Check size={14} />
        </button>
      ))}
    </div>
  );
}
