import { useCallback, useRef, useState, type DragEvent } from "react";
import { api } from "../../api/client";
import { ensureTrailingSlash, stripTrailingSlash } from "./directory-path-input";
import { parseDroppedDirectory } from "./dropped-directory";
import { resolveDirectoryCandidates } from "./picked-directory";

type DirectoryDropOptions = {
  /** 当前浏览目录，用于按目录名就近解析 */
  currentPath: string;
  /** 允许浏览的根目录 */
  roots: string[];
  /** 解析成功后选定目录 */
  onResolved: (path: string) => Promise<void>;
  /** 解析失败时的提示文案 */
  notFoundMessage: string;
};

/** 拖拽投放区的状态与事件处理。 */
export type DirectoryDrop = {
  active: boolean;
  error: string | null;
  reset: () => void;
  handlers: {
    onDragEnter: (event: DragEvent<HTMLElement>) => void;
    onDragOver: (event: DragEvent<HTMLElement>) => void;
    onDragLeave: () => void;
    onDrop: (event: DragEvent<HTMLElement>) => void;
  };
};

/**
 * 读取拖入项的目录名；明确是文件时返回空串。
 *
 * @param event 拖放事件
 * @returns 目录名或空串
 */
function droppedFolderName(event: DragEvent<HTMLElement>): string {
  const item = event.dataTransfer.items?.[0] as (DataTransferItem & { webkitGetAsEntry?: () => { isDirectory: boolean; name: string } | null }) | undefined;
  const entry = item?.webkitGetAsEntry?.();
  if (entry && !entry.isDirectory) return "";
  const file = event.dataTransfer.files[0] as (File & { path?: string }) | undefined;
  return entry?.name || file?.path?.split(/[\\/]/u).filter(Boolean).at(-1) || file?.name || "";
}

/**
 * 管理把本地文件夹拖进目录弹窗的投放流程。
 *
 * 文件管理器给出绝对路径时直接使用；浏览器只给目录名时，
 * 先看当前目录，再在允许的根下找第一个真实存在的同名目录。
 *
 * @param options 当前目录、根目录、成功回调与失败文案
 * @returns 投放状态与事件处理
 */
export function useDirectoryDrop({ currentPath, roots, onResolved, notFoundMessage }: DirectoryDropOptions): DirectoryDrop {
  const [active, setActive] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // 拖拽经过子元素时 enter/leave 成对出现，用深度避免高亮闪烁
  const depth = useRef(0);

  /**
   * 把拖入项解析成服务端目录。
   *
   * @param event 拖放事件
   * @returns 目录绝对路径；无法定位时返回 null
   */
  const resolve = async (event: DragEvent<HTMLElement>): Promise<string | null> => {
    const parsed = parseDroppedDirectory(event.dataTransfer.getData("text/uri-list"), event.dataTransfer.getData("text/plain"), droppedFolderName(event));
    if (parsed.path) return stripTrailingSlash(parsed.path);
    if (!parsed.name) return null;
    // 1. 候选顺序：当前目录下同名目录，其次各根目录下的同名目录
    const here = currentPath ? `${stripTrailingSlash(ensureTrailingSlash(currentPath))}/${parsed.name}` : "";
    const candidates = [...(here ? [here] : []), ...resolveDirectoryCandidates(parsed.name, roots)];
    for (const candidate of candidates) {
      try {
        await api.workspaces.browse(stripTrailingSlash(candidate));
        return stripTrailingSlash(candidate);
      } catch {
        // 2. 这个候选不存在，继续看下一个
      }
    }
    return null;
  };

  const handlers = {
    onDragEnter: (event: DragEvent<HTMLElement>) => {
      event.preventDefault();
      depth.current += 1;
      setActive(true);
    },
    onDragOver: (event: DragEvent<HTMLElement>) => {
      event.preventDefault();
      event.dataTransfer.dropEffect = "copy";
    },
    onDragLeave: () => {
      depth.current = Math.max(0, depth.current - 1);
      if (depth.current === 0) setActive(false);
    },
    onDrop: (event: DragEvent<HTMLElement>) => {
      event.preventDefault();
      depth.current = 0;
      setActive(false);
      void (async () => {
        const path = await resolve(event);
        if (!path) {
          setError(notFoundMessage);
          return;
        }
        setError(null);
        await onResolved(path);
      })();
    }
  };

  const reset = useCallback((): void => {
    depth.current = 0;
    setActive(false);
    setError(null);
  }, []);

  return { active, error, reset, handlers };
}
