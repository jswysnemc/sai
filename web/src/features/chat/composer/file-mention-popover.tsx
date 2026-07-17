import { FileText } from "lucide-react";
import { forwardRef, useEffect, useMemo, useRef, useState } from "react";
import type { KeyboardEvent } from "react";
import { api } from "../../../api/client";
import type { FileNode } from "../../../api/contracts";

type FileMentionPopoverProps = {
  open: boolean;
  onSelect: (path: string) => void;
  onClose: () => void;
};

/**
 * 将文件树递归扁平化为文件路径列表。
 *
 * @param nodes 文件树节点数组
 * @returns 全部文件的相对路径列表
 */
function flattenFilePaths(nodes: FileNode[]): string[] {
  const paths: string[] = [];
  for (const node of nodes) {
    if (node.kind === "file") paths.push(node.path);
    if (node.children.length > 0) paths.push(...flattenFilePaths(node.children));
  }
  return paths;
}

/**
 * 按模糊子串规则过滤路径。
 *
 * @param paths 全部路径
 * @param query 过滤关键词
 * @returns 匹配的路径列表
 */
function filterPaths(paths: string[], query: string): string[] {
  const keyword = query.trim().toLowerCase();
  if (!keyword) return paths;
  return paths.filter((path) => path.toLowerCase().includes(keyword));
}

/**
 * 渲染输入框上方的文件引用选择浮层。
 *
 * @param props 打开状态、选中回调和关闭回调
 * @returns 文件引用浮层，关闭时返回 null
 */
export const FileMentionPopover = forwardRef<HTMLDivElement, FileMentionPopoverProps>(function FileMentionPopover({ open, onSelect, onClose }, ref) {
  const [paths, setPaths] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);
  const [query, setQuery] = useState("");
  const [activeIndex, setActiveIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    // 1. 打开时重置过滤状态并聚焦过滤输入框
    setQuery("");
    setActiveIndex(0);
    requestAnimationFrame(() => inputRef.current?.focus());
    // 2. 拉取工作区文件树并扁平化为路径列表
    let cancelled = false;
    setLoading(true);
    api.workspace
      .tree()
      .then((nodes) => {
        if (!cancelled) setPaths(flattenFilePaths(nodes));
      })
      .catch(() => {
        if (!cancelled) setPaths([]);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [open]);

  const filtered = useMemo(() => filterPaths(paths, query), [paths, query]);

  useEffect(() => {
    // 过滤结果变化时收敛选中项范围
    setActiveIndex((index) => Math.min(index, Math.max(filtered.length - 1, 0)));
  }, [filtered.length]);

  useEffect(() => {
    // 选中项变化时保持其在滚动可视区内
    const item = listRef.current?.children[activeIndex] as HTMLElement | undefined;
    item?.scrollIntoView({ block: "nearest" });
  }, [activeIndex]);

  /**
   * 处理浮层内键盘导航、确认与关闭。
   *
   * @param event 过滤输入框键盘事件
   */
  const handleKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setActiveIndex((index) => Math.min(index + 1, filtered.length - 1));
      return;
    }
    if (event.key === "ArrowUp") {
      event.preventDefault();
      setActiveIndex((index) => Math.max(index - 1, 0));
      return;
    }
    if (event.key === "Enter") {
      event.preventDefault();
      const path = filtered[activeIndex];
      if (path) onSelect(path);
      return;
    }
    if (event.key === "Escape") {
      event.preventDefault();
      onClose();
    }
  };

  if (!open) return null;
  return (
    <div className="file-mention-popover" role="listbox" aria-label="选择引用文件" ref={ref}>
      <input
        ref={inputRef}
        className="file-mention-filter"
        value={query}
        placeholder="筛选文件路径"
        onChange={(event) => setQuery(event.target.value)}
        onKeyDown={handleKeyDown}
      />
      <div className="file-mention-list" ref={listRef}>
        {filtered.map((path, index) => (
          <button
            type="button"
            role="option"
            aria-selected={index === activeIndex}
            className={index === activeIndex ? "file-mention-item active" : "file-mention-item"}
            onMouseEnter={() => setActiveIndex(index)}
            onClick={() => onSelect(path)}
            key={path}
          >
            <FileText size={12} />
            <span>{path}</span>
          </button>
        ))}
        {filtered.length === 0 && <div className="file-mention-empty">{loading ? "正在加载文件树" : "没有匹配的文件"}</div>}
      </div>
    </div>
  );
});
