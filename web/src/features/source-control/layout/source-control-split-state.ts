import { useCallback, useEffect, useState } from "react";

export const SOURCE_CONTROL_SPLIT_MIN_LIST_WIDTH = 220;
export const SOURCE_CONTROL_SPLIT_MIN_DETAIL_WIDTH = 320;
export const SOURCE_CONTROL_SPLIT_DEFAULT_WIDTH = 320;
export const SOURCE_CONTROL_SPLIT_MAX_LIST_WIDTH = 720;

const STORAGE_KEY = "sai.source-control-list-width";

/**
 * 判断 Git 双栏是否需要切换为上下布局。
 *
 * @param containerWidth Git 分栏容器总宽度
 * @returns 容器无法同时容纳左右两侧最小宽度时返回 true
 */
export function shouldStackSourceControlSplit(containerWidth: number): boolean {
  return containerWidth < SOURCE_CONTROL_SPLIT_MIN_LIST_WIDTH + SOURCE_CONTROL_SPLIT_MIN_DETAIL_WIDTH;
}

/**
 * 约束 Git 列表栏宽度，并为右侧详情保留最小空间。
 *
 * @param width 请求设置的列表栏宽度
 * @param containerWidth Git 分栏容器总宽度
 * @returns 经过双侧最小宽度约束的列表栏宽度
 */
export function clampSourceControlListWidth(width: number, containerWidth: number): number {
  const maximum = Math.max(
    SOURCE_CONTROL_SPLIT_MIN_LIST_WIDTH,
    Math.min(SOURCE_CONTROL_SPLIT_MAX_LIST_WIDTH, containerWidth - SOURCE_CONTROL_SPLIT_MIN_DETAIL_WIDTH)
  );
  return Math.min(maximum, Math.max(SOURCE_CONTROL_SPLIT_MIN_LIST_WIDTH, width));
}

/**
 * 解析本地保存的 Git 列表栏宽度。
 *
 * @param raw 本地存储中的原始值
 * @returns 可用于初始布局的列表栏宽度
 */
export function parseSourceControlListWidth(raw: string | null): number {
  if (raw === null) return SOURCE_CONTROL_SPLIT_DEFAULT_WIDTH;
  const width = Number(raw);
  if (!Number.isFinite(width)) return SOURCE_CONTROL_SPLIT_DEFAULT_WIDTH;
  return Math.min(SOURCE_CONTROL_SPLIT_MAX_LIST_WIDTH, Math.max(SOURCE_CONTROL_SPLIT_MIN_LIST_WIDTH, width));
}

/**
 * 管理 Git 分栏宽度及本地持久化。
 *
 * @returns 当前宽度和按容器约束宽度的方法
 */
export function useSourceControlSplitState() {
  const [listWidth, setListWidth] = useState(() => parseSourceControlListWidth(window.localStorage.getItem(STORAGE_KEY)));

  useEffect(() => {
    window.localStorage.setItem(STORAGE_KEY, String(listWidth));
  }, [listWidth]);

  /**
   * 按当前容器宽度更新列表栏宽度。
   *
   * @param width 请求设置的列表栏宽度
   * @param containerWidth Git 分栏容器总宽度
   */
  const resize = useCallback((width: number, containerWidth: number) => {
    setListWidth(clampSourceControlListWidth(width, containerWidth));
  }, []);

  /**
   * 容器尺寸变化时重新约束现有宽度。
   *
   * @param containerWidth Git 分栏容器总宽度
   */
  const constrain = useCallback((containerWidth: number) => {
    setListWidth((current) => clampSourceControlListWidth(current, containerWidth));
  }, []);

  return { listWidth, resize, constrain };
}
