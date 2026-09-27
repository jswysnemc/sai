import { ArrowLeft, HardDrive, Loader2, Search } from "../../shared/ui/icons";
import { Button } from "../../shared/ui/button/button";
import { useI18n } from "../i18n/use-i18n";
import { ensureTrailingSlash } from "./directory-path-input";
import type { DirectoryBrowser } from "./use-directory-browser";

type DirectoryToolbarProps = {
  browser: DirectoryBrowser;
};

/**
 * 渲染目录弹窗的双层控制条。
 *
 * 第一层是等宽路径导航（上级按钮、完整路径输入、加载指示）；
 * 第二层左侧是根目录快捷跳转，右侧是只过滤当前目录的搜索框。
 *
 * @param props 目录浏览状态
 * @returns 双层控制条
 */
export function DirectoryToolbar({ browser }: DirectoryToolbarProps) {
  const { t } = useI18n();
  const roots = browser.listing.data?.roots ?? [];

  return (
    <div className="directory-toolbar">
      <div className="directory-path-row">
        <Button
          variant="ghost"
          size="icon"
          className="directory-up-button"
          onClick={browser.goToParent}
          disabled={!browser.parent}
          aria-label={t("Go to parent directory", "返回上级目录")}
          title={t("Go to parent directory", "返回上级目录")}
        >
          <ArrowLeft size={14} aria-hidden />
        </Button>
        <input
          ref={browser.inputRef}
          className="directory-path-input"
          value={browser.pathInput}
          placeholder={t("Full path, e.g. /home/you or C:/Users/you — Enter to navigate", "输入完整路径（如 /home/you 或 C:/Users/you），回车跳转")}
          aria-label={t("Directory path", "目录路径")}
          spellCheck={false}
          autoComplete="off"
          onChange={(event) => browser.changePathInput(event.target.value)}
          onKeyDown={browser.handlePathKeyDown}
        />
        {browser.listing.isFetching && <Loader2 size={14} className="spin directory-path-spinner" aria-hidden />}
      </div>
      <div className="directory-filter-row">
        {roots.length > 0 && (
          <div className="directory-roots" role="group" aria-label={t("Roots", "根目录")}>
            {roots.map((root) => {
              const active = ensureTrailingSlash(root.path) === browser.activeRootPath;
              return (
                <Button
                  key={root.path}
                  variant="ghost"
                  size="small"
                  className={`directory-root${active ? " is-active" : ""}`}
                  aria-pressed={active}
                  onClick={() => browser.enterDirectory(root.path)}
                  title={root.path}
                >
                  <span className="icon-label">
                    <HardDrive size={14} aria-hidden />
                    <span>{root.name}</span>
                  </span>
                </Button>
              );
            })}
          </div>
        )}
        <label className="directory-filter">
          <Search size={14} aria-hidden />
          <input
            value={browser.filter}
            placeholder={t("Filter subdirectories", "过滤子目录")}
            aria-label={t("Filter subdirectories", "过滤子目录")}
            spellCheck={false}
            autoComplete="off"
            onChange={(event) => browser.changeFilter(event.target.value)}
            onKeyDown={browser.handleFilterKeyDown}
          />
        </label>
      </div>
    </div>
  );
}
