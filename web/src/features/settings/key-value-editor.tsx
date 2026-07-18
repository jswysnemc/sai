import { Plus, Trash2 } from "lucide-react";
import { useI18n } from "../i18n/use-i18n";

type KeyValueEditorProps = {
  value: Record<string, string>;
  keyPlaceholder?: string;
  valuePlaceholder?: string;
  addLabel?: string;
  onChange: (value: Record<string, string>) => void;
};

/**
 * 编辑字符串键值表，用于 HTTP headers / MCP env 等配置。
 *
 * @param props 当前映射与变更回调
 * @returns 键值编辑器
 */
export function KeyValueEditor({
  value,
  keyPlaceholder,
  valuePlaceholder,
  addLabel,
  onChange
}: KeyValueEditorProps) {
  const { t } = useI18n();
  const entries = Object.entries(value ?? {});

  const commit = (next: Array<[string, string]>) => {
    const mapped: Record<string, string> = {};
    for (const [key, item] of next) {
      const trimmed = key.trim();
      if (!trimmed) continue;
      mapped[trimmed] = item;
    }
    onChange(mapped);
  };

  return (
    <div className="key-value-editor">
      {entries.length === 0 && (
        <div className="key-value-empty">{t("No entries yet", "暂无条目")}</div>
      )}
      {entries.map(([key, item], index) => (
        <div className="key-value-row" key={`${index}-${key}`}>
          <input
            value={key}
            onChange={(event) => {
              const next = [...entries] as Array<[string, string]>;
              next[index] = [event.target.value, item];
              commit(next);
            }}
            placeholder={keyPlaceholder ?? t("Key", "键")}
            spellCheck={false}
          />
          <input
            value={item}
            onChange={(event) => {
              const next = [...entries] as Array<[string, string]>;
              next[index] = [key, event.target.value];
              commit(next);
            }}
            placeholder={valuePlaceholder ?? t("Value", "值")}
            spellCheck={false}
          />
          <button
            type="button"
            className="settings-secondary"
            aria-label={t("Remove entry", "删除条目")}
            onClick={() => commit(entries.filter((_, i) => i !== index))}
          >
            <Trash2 size={14} />
          </button>
        </div>
      ))}
      <button
        type="button"
        className="settings-secondary key-value-add"
        onClick={() => commit([...entries, ["", ""]])}
      >
        <Plus size={14} />{addLabel ?? t("Add entry", "添加条目")}
      </button>
    </div>
  );
}
