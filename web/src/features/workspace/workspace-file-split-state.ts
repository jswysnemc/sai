import { useCallback, useEffect, useState } from "react";

export const WORKSPACE_FILE_TREE_MIN_WIDTH = 220;
export const WORKSPACE_EDITOR_MIN_WIDTH = 320;
export const WORKSPACE_FILE_TREE_DEFAULT_WIDTH = 320;
export const WORKSPACE_FILE_TREE_MAX_WIDTH = 560;

const STORAGE_KEY = "sai.workspace-file-tree-width";

/**
 * 判断工作区宽度是否不足以并排显示编辑器和文件树。
 *
 * @param containerWidth 文件工作区总宽度
 * @returns 需要使用覆盖式文件树时返回 true
 */
export function shouldOverlayWorkspaceFileTree(containerWidth: number): boolean {
  return containerWidth < WORKSPACE_FILE_TREE_MIN_WIDTH + WORKSPACE_EDITOR_MIN_WIDTH;
}

/**
 * 约束文件树宽度，并为编辑器保留最小空间。
 *
 * @param width 请求设置的文件树宽度
 * @param containerWidth 文件工作区总宽度
 * @returns 经过两侧最小宽度约束的文件树宽度
 */
export function clampWorkspaceFileTreeWidth(width: number, containerWidth: number): number {
  const maximum = Math.max(
    WORKSPACE_FILE_TREE_MIN_WIDTH,
    Math.min(WORKSPACE_FILE_TREE_MAX_WIDTH, containerWidth - WORKSPACE_EDITOR_MIN_WIDTH)
  );
  return Math.min(maximum, Math.max(WORKSPACE_FILE_TREE_MIN_WIDTH, width));
}

/**
 * 解析本地保存的文件树宽度。
 *
 * @param raw 本地存储中的原始值
 * @returns 可用于初始布局的文件树宽度
 */
export function parseWorkspaceFileTreeWidth(raw: string | null): number {
  if (raw === null) return WORKSPACE_FILE_TREE_DEFAULT_WIDTH;
  const width = Number(raw);
  if (!Number.isFinite(width)) return WORKSPACE_FILE_TREE_DEFAULT_WIDTH;
  return Math.min(
    WORKSPACE_FILE_TREE_MAX_WIDTH,
    Math.max(WORKSPACE_FILE_TREE_MIN_WIDTH, width)
  );
}

/**
 * 管理文件树宽度及本地持久化。
 *
 * @returns 当前宽度、调整方法和容器约束方法
 */
export function useWorkspaceFileSplitState() {
  const [treeWidth, setTreeWidth] = useState(() =>
    parseWorkspaceFileTreeWidth(window.localStorage.getItem(STORAGE_KEY))
  );

  useEffect(() => {
    window.localStorage.setItem(STORAGE_KEY, String(treeWidth));
  }, [treeWidth]);

  /**
   * 按当前容器宽度更新文件树宽度。
   *
   * @param width 请求设置的文件树宽度
   * @param containerWidth 文件工作区总宽度
   * @returns 无
   */
  const resize = useCallback((width: number, containerWidth: number) => {
    setTreeWidth(clampWorkspaceFileTreeWidth(width, containerWidth));
  }, []);

  /**
   * 容器尺寸变化时重新约束现有宽度。
   *
   * @param containerWidth 文件工作区总宽度
   * @returns 无
   */
  const constrain = useCallback((containerWidth: number) => {
    setTreeWidth((current) => clampWorkspaceFileTreeWidth(current, containerWidth));
  }, []);

  return { treeWidth, resize, constrain };
}
