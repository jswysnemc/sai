import { ChevronLeft } from "lucide-react";

/** 会话行悬停「显示文件树」时打开左栏滑盖。 */
export const OPEN_SIDEBAR_FILE_TREE_EVENT = "sai:open-sidebar-file-tree";
import { Button } from "../../shared/ui/button/button";
import { FileTree } from "../workspace/file-tree";
import { useI18n } from "../i18n/use-i18n";
import "./sidebar-file-tree-cover.css";

type SidebarFileTreeCoverProps = {
  open: boolean;
  workspaceLabel?: string;
  selectedFile: string | null;
  onSelectFile: (path: string) => void;
  onClearFile: () => void;
  onClose: () => void;
};

/**
 * 在左栏上滑入文件树，盖住项目和任务。
 *
 * @param props 打开状态、当前文件和关闭回调
 * @returns 文件树滑盖
 */
export function SidebarFileTreeCover({ open, workspaceLabel, selectedFile, onSelectFile, onClearFile, onClose }: SidebarFileTreeCoverProps) {
  const { t } = useI18n();
  return (
    <div className={`sidebar-file-tree-cover${open ? " is-open" : ""}`} aria-hidden={!open}>
      <div className="sidebar-file-tree-cover-bar">
        <Button variant="ghost" onClick={onClose} aria-label={t("Back to tasks", "返回任务")}>
          <ChevronLeft size={16} />
          <span>{t("Back to tasks", "返回任务")}</span>
        </Button>
      </div>
      <FileTree
        selectedFile={selectedFile}
        onSelectFile={onSelectFile}
        onClearFile={onClearFile}
        showHeading={false}
        workspaceLabel={workspaceLabel}
        searchPlaceholder={t("Search files...", "搜索文件...")}
      />
    </div>
  );
}
