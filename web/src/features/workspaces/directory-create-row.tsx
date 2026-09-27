import { useState } from "react";
import { FolderPlus } from "../../shared/ui/icons";
import { api } from "../../api/client";
import { toDisplayError } from "../../api/api-error";
import { useI18n } from "../i18n/use-i18n";

type DirectoryCreateRowProps = {
  /** 新目录的父目录 */
  parentPath: string;
  /** 创建成功后回传新目录路径 */
  onCreated: (path: string) => Promise<void>;
  onCancel: () => void;
};

/**
 * 渲染列表顶部的新建文件夹输入行：回车创建，Esc 取消。
 *
 * @param props 父目录、创建成功与取消回调
 * @returns 新建文件夹行
 */
export function DirectoryCreateRow({ parentPath, onCreated, onCancel }: DirectoryCreateRowProps) {
  const { t } = useI18n();
  const [name, setName] = useState("");
  const [error, setError] = useState<Error | null>(null);

  /**
   * 在父目录下创建子目录。
   *
   * @returns 创建完成后的 Promise
   */
  const create = async (): Promise<void> => {
    const trimmed = name.trim();
    if (!parentPath || !trimmed) return;
    setError(null);
    try {
      const entry = await api.workspaces.createDirectory(parentPath, trimmed);
      await onCreated(entry.path);
    } catch (cause) {
      setError(toDisplayError(cause, "Failed to create directory", "创建目录失败"));
    }
  };

  return (
    <>
      <div className="directory-row directory-create-row">
        <span className="icon-label directory-row-label">
          <FolderPlus size={14} aria-hidden />
          <input
            autoFocus
            value={name}
            placeholder={t("New folder name; Enter to create, Escape to cancel", "新文件夹名称，回车创建，Esc 取消")}
            aria-label={t("New folder name", "新文件夹名称")}
            spellCheck={false}
            onChange={(event) => setName(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") void create();
              if (event.key === "Escape") {
                event.preventDefault();
                onCancel();
              }
            }}
          />
        </span>
      </div>
      {error && <div className="directory-message is-error" role="alert">{error.message}</div>}
    </>
  );
}
