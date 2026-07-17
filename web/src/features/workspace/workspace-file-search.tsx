import { Search, X } from "lucide-react";
import "./workspace-file-search.css";
import { useI18n } from "../i18n/use-i18n";

type WorkspaceFileSearchProps = {
  value: string;
  onChange: (value: string) => void;
};

/**
 * 渲染工作区文件过滤输入框。
 *
 * @param props 当前关键词和更新回调
 * @returns 文件搜索控件
 */
export function WorkspaceFileSearch({ value, onChange }: WorkspaceFileSearchProps) {
  const { t } = useI18n();
  return (
    <label className="workspace-file-search">
      <Search size={13} aria-hidden="true" />
      <input
        value={value}
        onChange={(event) => onChange(event.target.value)}
        placeholder={t("Filter files", "筛选文件")}
        aria-label={t("Filter workspace files", "筛选工作区文件")}
        spellCheck={false}
      />
      {value && (
        <button type="button" onClick={() => onChange("")} aria-label={t("Clear file filter", "清除文件筛选")}>
          <X size={12} />
        </button>
      )}
    </label>
  );
}
