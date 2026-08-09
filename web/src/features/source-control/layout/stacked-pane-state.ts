import { useCallback, useState } from "react";

/**
 * 窄屏下 Git 视图的堆叠切换状态。
 *
 * 宽屏保持左右分栏，窄屏一次只呈现列表或详情之一。切换时记录方向，
 * 供进场动画区分前进（列表进详情）与后退（详情回列表）。
 */
export type StackedPane = "list" | "detail";
export type StackedDirection = "forward" | "back";

export type StackedPaneState = {
  pane: StackedPane;
  direction: StackedDirection;
};

export const INITIAL_STACKED_PANE_STATE: StackedPaneState = {
  pane: "list",
  direction: "forward"
};

/**
 * 计算切换到目标区域后的堆叠状态。
 *
 * 方向由切换前后的区域推导：进入详情为前进，回到列表为后退；
 * 目标与当前一致时保持原状态，避免重复触发进场动画。
 *
 * @param current 当前堆叠状态
 * @param next 目标区域
 * @returns 切换后的堆叠状态
 */
export function reduceStackedPane(current: StackedPaneState, next: StackedPane): StackedPaneState {
  if (current.pane === next) return current;
  return { pane: next, direction: next === "detail" ? "forward" : "back" };
}

/**
 * 管理窄屏堆叠切换状态。
 *
 * @returns 当前区域、方向及切换方法
 */
export function useStackedPaneState() {
  const [state, setState] = useState<StackedPaneState>(INITIAL_STACKED_PANE_STATE);

  /**
   * 切换到指定区域。
   *
   * @param next 目标区域
   */
  const showPane = useCallback((next: StackedPane) => {
    setState((current) => reduceStackedPane(current, next));
  }, []);

  /**
   * 回到列表区域。
   */
  const showList = useCallback(() => showPane("list"), [showPane]);

  /**
   * 进入详情区域。
   */
  const showDetail = useCallback(() => showPane("detail"), [showPane]);

  return { pane: state.pane, direction: state.direction, showPane, showList, showDetail };
}
