import { Search, X } from "lucide-react";
import "./workspace-file-search.css";
import { useI18n } from "../i18n/use-i18n";

type WorkspaceFileSearchProps = {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  autoFocus?: boolean;
};

/**
 * 渲染工作区文件过滤输入框。
 *
 * @param props 当前关键词和更新回调
 * @returns 文件搜索控件
 */
export function WorkspaceFileSearch({ value, onChange, placeholder, autoFocus = false }: WorkspaceFileSearchProps) {
  const { t } = useI18n();
  const label = placeholder ?? t("Filter files", "筛选文件");
  return (
    <label className="workspace-file-search">
      <Search size={13} aria-hidden="true" />
      <input
        value={value}
        onChange={(event) => onChange(event.target.value)}
        placeholder={label}
        aria-label={t("Filter workspace files", "筛选工作区文件")}
        spellCheck={false}
        autoFocus={autoFocus}
      />
      {value && (
        <button type="button" onClick={() => onChange("")} aria-label={t("Clear file filter", "清除文件筛选")}>
          <X size={12} />
        </button>
      )}
    </label>
  );
}
