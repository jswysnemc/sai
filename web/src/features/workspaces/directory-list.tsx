import type { ReactNode } from "react";
import { ArrowLeft, Folder, FolderInput, GitBranch } from "../../shared/ui/icons";
import { Button } from "../../shared/ui/button/button";
import { useI18n } from "../i18n/use-i18n";
import type { DirectoryBrowser } from "./use-directory-browser";

type DirectoryListProps = {
  browser: DirectoryBrowser;
  /** 拖拽悬停中，列表以内置边框线提示可投放 */
  dropActive: boolean;
  /** 置顶显示的提示与错误 */
  messages: (string | null | undefined)[];
  /** 新建文件夹行，放在上级目录行之后 */
  createRow?: ReactNode;
};

/**
 * 渲染目录列表：上级目录行、新建行、子目录行与空态。
 *
 * 行高固定 28px；高亮行右侧出现"选择"，双击直接选定。
 *
 * @param props 目录浏览状态、投放状态、提示与新建行
 * @returns 可滚动的目录列表
 */
export function DirectoryList({ browser, dropActive, messages, createRow }: DirectoryListProps) {
  const { t } = useI18n();
  const { entries, highlight, listing, filter } = browser;
  const browseError = browseErrorMessage(listing.error?.message, t);

  return (
    <div
      ref={browser.listRef}
      className={`directory-list${dropActive ? " is-drop-target" : ""}`}
      role="listbox"
      aria-label={t("Subdirectories", "子目录")}
    >
      {dropActive && (
        <div className="directory-message is-drop">
          <span className="icon-label">
            <FolderInput size={14} aria-hidden />
            <span>{t("Drop to open this folder", "松开以打开这个文件夹")}</span>
          </span>
        </div>
      )}
      {[browseError, ...messages].filter(Boolean).map((message) => (
        <div key={message} className="directory-message is-error" role="alert">{message}</div>
      ))}
      {browser.parent && (
        <div
          id="directory-row--1"
          role="option"
          aria-selected={highlight === -1}
          data-row-index={-1}
          className={`directory-row${highlight === -1 ? " is-highlighted" : ""}`}
          onClick={browser.goToParent}
          onMouseEnter={() => browser.setHighlight(-1)}
        >
          <span className="icon-label directory-row-label">
            <ArrowLeft size={14} aria-hidden />
            <span className="directory-row-name">{t(".. (parent directory)", "..（上级目录）")}</span>
          </span>
        </div>
      )}
      {createRow}
      {entries.map((entry, index) => (
        <div
          key={entry.path}
          id={`directory-row-${index}`}
          role="option"
          aria-selected={highlight === index}
          data-row-index={index}
          className={`directory-row${highlight === index ? " is-highlighted" : ""}`}
          onClick={() => browser.enterDirectory(entry.path)}
          onDoubleClick={() => void browser.submit(entry.path)}
          onMouseEnter={() => browser.setHighlight(index)}
        >
          <span className="icon-label directory-row-label">
            <Folder size={14} aria-hidden />
            <span className="directory-row-name">{entry.name}</span>
            {entry.git_repository && (
              <span className="icon-label directory-git">
                <GitBranch size={14} aria-hidden />
                <span>Git</span>
              </span>
            )}
          </span>
          {highlight === index && (
            <Button
              variant="primary"
              size="small"
              className="directory-row-select"
              tabIndex={-1}
              onClick={(event) => {
                event.stopPropagation();
                void browser.submit(entry.path);
              }}
            >
              {t("Select", "选择")}
            </Button>
          )}
        </div>
      ))}
      {!listing.error && entries.length === 0 && !listing.isLoading && (
        <div className="directory-empty">
          {filter
            ? t(`No directories match “${filter}”`, `没有匹配“${filter}”的目录`)
            : t("The current directory has no browsable subdirectories", "当前目录没有可浏览的子目录")}
        </div>
      )}
    </div>
  );
}

/**
 * 把服务端英文读取错误换成可执行的本地化提示。
 *
 * @param message 原始错误信息
 * @param t 双语文案取值函数
 * @returns 提示文案；没有错误时返回 null
 */
function browseErrorMessage(message: string | undefined, t: (en: string, zh: string) => string): string | null {
  if (!message) return null;
  if (!message.includes("failed to read directory")) return message;
  return t(
    "This directory cannot be read (the server may lack permission). Edit the path above or go back to another directory.",
    "无法读取该目录（服务端可能没有权限）。请修改上方路径或改到其他目录。"
  );
}
