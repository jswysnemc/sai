import { useCallback, useEffect, useRef, useState } from "react";
import { Plus, Trash2 } from "lucide-react";
import { useI18n } from "../i18n/use-i18n";

type KeyValueEditorProps = {
  value: Record<string, string>;
  keyPlaceholder?: string;
  valuePlaceholder?: string;
  addLabel?: string;
  onChange: (value: Record<string, string>) => void;
};

type Row = {
  id: number;
  key: string;
  value: string;
};

let nextRowId = 1;

/**
 * 将外部 Record 转为本地行数组。
 *
 * @param record 键值映射
 * @returns 行数组
 */
function recordToRows(record: Record<string, string>): Row[] {
  return Object.entries(record).map(([key, value]) => ({
    id: nextRowId++,
    key,
    value
  }));
}

/**
 * 将本地行数组同步为 Record，过滤空 key。
 *
 * @param rows 行数组
 * @returns 键值映射
 */
function rowsToRecord(rows: Row[]): Record<string, string> {
  const mapped: Record<string, string> = {};
  for (const row of rows) {
    const trimmed = row.key.trim();
    if (!trimmed) continue;
    mapped[trimmed] = row.value;
  }
  return mapped;
}

/**
 * 编辑字符串键值表，用于 HTTP headers / MCP env 等配置。
 * 使用本地行状态管理，允许空行存在以便用户编辑。
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
  const [rows, setRows] = useState<Row[]>(() => recordToRows(value ?? {}));
  // 记录上次同步给外部的快照，避免外部回写时覆盖正在编辑的空行
  const lastSyncedRef = useRef<string>(JSON.stringify(rowsToRecord(rows)));

  // 1. 外部 value 变化时（如保存后服务端回写），同步本地行
  useEffect(() => {
    const externalJson = JSON.stringify(value ?? {});
    if (externalJson !== lastSyncedRef.current) {
      const newRows = recordToRows(value ?? {});
      setRows(newRows);
      lastSyncedRef.current = JSON.stringify(rowsToRecord(newRows));
    }
  }, [value]);

  /**
   * 更新行并同步给父组件。
   *
   * @param updater 行数组更新函数
   */
  const updateRows = useCallback((updater: (prev: Row[]) => Row[]) => {
    setRows((prev) => {
      const next = updater(prev);
      const record = rowsToRecord(next);
      const recordJson = JSON.stringify(record);
      // 2. 仅在有效键值映射变化时通知父组件
      if (recordJson !== lastSyncedRef.current) {
        lastSyncedRef.current = recordJson;
        onChange(record);
      }
      return next;
    });
  }, [onChange]);

  /**
   * 更新指定行的键或值。
   *
   * @param id 行标识
   * @param field 字段名
   * @param text 新文本
   */
  const handleFieldChange = (id: number, field: "key" | "value", text: string) => {
    updateRows((prev) =>
      prev.map((row) => (row.id === id ? { ...row, [field]: text } : row))
    );
  };

  /**
   * 删除指定行。
   *
   * @param id 行标识
   */
  const handleRemove = (id: number) => {
    updateRows((prev) => prev.filter((row) => row.id !== id));
  };

  /** 添加一个空行供用户编辑。 */
  const handleAdd = () => {
    setRows((prev) => [...prev, { id: nextRowId++, key: "", value: "" }]);
  };

  return (
    <div className="key-value-editor">
      {rows.length === 0 && (
        <div className="key-value-empty">{t("No entries yet", "暂无条目")}</div>
      )}
      {rows.map((row) => (
        <div className="key-value-row" key={row.id}>
          <input
            value={row.key}
            onChange={(event) => handleFieldChange(row.id, "key", event.target.value)}
            placeholder={keyPlaceholder ?? t("Key", "键")}
            spellCheck={false}
          />
          <input
            value={row.value}
            onChange={(event) => handleFieldChange(row.id, "value", event.target.value)}
            placeholder={valuePlaceholder ?? t("Value", "值")}
            spellCheck={false}
          />
          <button
            type="button"
            className="settings-secondary"
            aria-label={t("Remove entry", "删除条目")}
            onClick={() => handleRemove(row.id)}
          >
            <Trash2 size={14} />
          </button>
        </div>
      ))}
      <button
        type="button"
        className="settings-secondary key-value-add"
        onClick={handleAdd}
      >
        <Plus size={14} />{addLabel ?? t("Add entry", "添加条目")}
      </button>
    </div>
  );
}
