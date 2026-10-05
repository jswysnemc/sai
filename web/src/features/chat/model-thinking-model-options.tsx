import { Check, ChevronLeft, ChevronRight, Clock3, Search } from "../../shared/ui/icons";
import { useState } from "react";
import { ModelIcon } from "../../shared/ui/model-icon";
import type { ChatModelChoice } from "./chat-model-options";
import { groupChatModelChoices } from "./chat-model-options";
import { useI18n } from "../i18n/use-i18n";

type ModelOptionsProps = {
  choices: ChatModelChoice[];
  selection: ChatModelChoice | null;
  pendingSelection?: ChatModelChoice | null;
  query: string;
  onQueryChange: (value: string) => void;
  onSelect: (choice: ChatModelChoice) => void;
};

/**
 * 渲染按供应商分组的二级模型列表。
 *
 * @param props 已过滤模型、当前选择、待生效选择、搜索状态和选择回调
 * @returns 模型二级菜单
 */
export function ModelOptions({ choices, selection, pendingSelection, query, onQueryChange, onSelect }: ModelOptionsProps) {
  const { t } = useI18n();
  const groups = groupChatModelChoices(choices);
  const [vendorId, setVendorId] = useState(selection?.providerId ?? groups[0]?.providerId ?? "");
  const searching = query.trim().length > 0;
  const showVendorList = !searching && groups.length > 1 && !groups.some((group) => group.providerId === vendorId);
  const activeGroup = groups.find((group) => group.providerId === vendorId);

  return (
    <>
      <label className="model-thinking-search">
        <Search size={14} />
        <input value={query} onChange={(event) => onQueryChange(event.target.value)} placeholder={t("Search models or providers", "搜索模型或供应商")} aria-label={t("Search models or providers", "搜索模型或供应商")} autoFocus />
      </label>
      {showVendorList ? (
        <div className="model-thinking-option-list" role="listbox" aria-label={t("Choose provider", "选择供应商")}>
          {groups.map((group) => (
            <button
              type="button"
              role="option"
              aria-selected={group.providerId === selection?.providerId}
              className={`model-thinking-vendor${group.providerId === selection?.providerId ? " active" : ""}`}
              key={group.providerId}
              onClick={() => setVendorId(group.providerId)}
            >
              <span className="model-thinking-option-main">
                <ModelIcon model={group.models[0]?.model ?? group.providerName} size={16} />
                <strong>{group.providerName}</strong>
              </span>
              <small>{t(`${group.models.length} models`, `${group.models.length} 个模型`)}</small>
              <ChevronRight size={14} />
            </button>
          ))}
        </div>
      ) : (
        <div className="model-thinking-option-list" role="listbox" aria-label={t("Choose model", "选择模型")}>
          {!searching && groups.length > 1 && activeGroup && (
            <button type="button" className="model-thinking-back" onClick={() => setVendorId("")}>
              <ChevronLeft size={14} />
              <span>{activeGroup.providerName}</span>
            </button>
          )}
          {(searching ? groups : activeGroup ? [activeGroup] : groups).map((group) => (
            <VendorModels
              key={group.providerId}
              group={group}
              showHeader={searching && groups.length > 1}
              selection={selection}
              pendingSelection={pendingSelection}
              onSelect={onSelect}
            />
          ))}
          {choices.length === 0 && <div className="model-thinking-empty">{t("No matching models", "没有匹配的模型")}</div>}
        </div>
      )}
    </>
  );
}

/**
 * 渲染一个供应商下的模型选项。
 *
 * @param props 分组、是否显示标题、当前选择和选择回调
 * @returns 分组标题与模型按钮
 */
function VendorModels({
  group,
  showHeader,
  selection,
  pendingSelection,
  onSelect
}: {
  group: { providerId: string; providerName: string; models: ChatModelChoice[] };
  showHeader: boolean;
  selection: ChatModelChoice | null;
  pendingSelection?: ChatModelChoice | null;
  onSelect: (choice: ChatModelChoice) => void;
}) {
  const { t } = useI18n();
  return (
    <>
      {showHeader && <div className="model-thinking-group">{group.providerName}</div>}
      {group.models.map((choice) => {
        const active = choice.providerId === selection?.providerId && choice.model === selection.model;
        const isPending = choice.providerId === pendingSelection?.providerId && choice.model === pendingSelection.model;
        return (
          <button type="button" role="option" aria-selected={active} aria-label={`${choice.model}，${choice.providerName}`} className={active ? "active" : isPending ? "pending" : ""} key={`${choice.providerId}-${choice.model}`} onClick={() => onSelect(choice)}>
            <span className="model-thinking-option-main"><ModelIcon model={choice.model} size={16} /><strong>{choice.model}</strong></span>
            {isPending
              ? <small className="model-thinking-option-pending"><Clock3 size={12} aria-hidden />{t("Next turn", "下轮生效")}</small>
              : showHeader ? <small>{choice.providerName}</small> : <small />}
            <Check size={14} />
          </button>
        );
      })}
    </>
  );
}
