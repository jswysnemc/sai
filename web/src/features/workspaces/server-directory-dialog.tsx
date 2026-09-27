import { useEffect, useState } from "react";
import { FolderPlus, Plus } from "../../shared/ui/icons";
import { Button } from "../../shared/ui/button/button";
import { Modal } from "../../shared/ui/dialog/modal";
import { useI18n } from "../i18n/use-i18n";
import { DirectoryCreateRow } from "./directory-create-row";
import { DirectoryList } from "./directory-list";
import { DirectoryToolbar } from "./directory-toolbar";
import { useDirectoryBrowser } from "./use-directory-browser";
import { useDirectoryDrop } from "./use-directory-drop";
import "./server-directory-dialog.css";

type ServerDirectoryDialogProps = {
  open: boolean;
  title?: string;
  description?: string;
  currentLabel?: string;
  pendingLabel?: string;
  onClose: () => void;
  onSelect: (path: string) => Promise<void>;
};

/**
 * 渲染服务端目录浏览和选择对话框。
 *
 * 结构：双层控制条（路径导航 / 根目录与过滤）、28px 行高的目录列表、底栏操作。
 * 路径与搜索职责分离：路径栏负责跳转，过滤框只筛选当前目录的子目录。
 * 把本地文件夹拖进弹窗即可直接选定。
 *
 * @param props 打开状态、文案覆盖、关闭与目录选择回调
 * @returns 服务端目录选择弹层
 */
export function ServerDirectoryDialog(props: ServerDirectoryDialogProps) {
  const { t } = useI18n();
  const browser = useDirectoryBrowser(props.open, props.onSelect, props.onClose);
  const [creating, setCreating] = useState(false);
  const drop = useDirectoryDrop({
    currentPath: browser.currentPath,
    roots: (browser.listing.data?.roots ?? []).map((root) => root.path),
    onResolved: browser.submit,
    notFoundMessage: t("Could not locate that folder on the server. Type its path above.", "没能在服务器上定位这个文件夹，请在上方输入路径。")
  });

  // 1. 每次打开都回到初始状态
  const { reset } = drop;
  useEffect(() => {
    if (!props.open) return;
    setCreating(false);
    reset();
  }, [props.open, reset]);

  return (
    <Modal
      open={props.open}
      title={props.title ?? t("Open server workspace", "打开服务端工作区")}
      description={props.description ?? t("Choose a directory on the server, or drop a folder into this window.", "选择服务器上的目录，或把文件夹拖进这个窗口。")}
      size="medium"
      className="directory-picker"
      flush
      onClose={props.onClose}
    >
      <div className="server-directory-dialog" {...drop.handlers}>
        <DirectoryToolbar browser={browser} />
        <DirectoryList
          browser={browser}
          dropActive={drop.active}
          messages={[drop.error, browser.submitError?.message]}
          createRow={creating && browser.currentPath ? (
            <DirectoryCreateRow
              parentPath={browser.currentPath}
              onCancel={() => setCreating(false)}
              onCreated={async (path) => {
                await browser.listing.refetch();
                setCreating(false);
                browser.enterDirectory(path);
              }}
            />
          ) : undefined}
        />
        <footer className="directory-footer">
          <Button variant="ghost" size="small" disabled={!browser.listing.data} onClick={() => setCreating((value) => !value)}>
            <span className="icon-label">
              <FolderPlus size={14} aria-hidden />
              <span>{t("New folder", "新建文件夹")}</span>
            </span>
          </Button>
          <Button variant="primary" size="small" onClick={browser.submitCurrent} disabled={browser.submitting || !browser.currentPath}>
            <span className="icon-label">
              <Plus size={14} aria-hidden />
              <span>
                {browser.submitting
                  ? props.pendingLabel ?? t("Opening", "正在打开")
                  : props.currentLabel ?? t("Open current directory", "打开当前目录")}
              </span>
            </span>
          </Button>
        </footer>
      </div>
    </Modal>
  );
}
