import type { RunModelSelection } from "../../api/contracts";

/** 一次模型点选的生效方式。 */
export type ModelSelectAction =
  | { kind: "apply"; selection: RunModelSelection }
  | { kind: "stage"; selection: RunModelSelection }
  | { kind: "unstage" };

/**
 * 判断两个模型选择指向同一供应商与模型。
 *
 * @param left 待比较选择
 * @param right 待比较选择
 * @returns 供应商与模型都一致时为 true
 */
export function isSameModelSelection(
  left: RunModelSelection | null | undefined,
  right: RunModelSelection | null | undefined
): boolean {
  return Boolean(
    left
    && right
    && left.providerId === right.providerId
    && left.model === right.model
  );
}

/**
 * 决定用户点选模型后的生效方式。
 *
 * 空闲时立即应用；运行中不打断当前 turn，暂存为待生效选择，
 * 待本轮结束后自动应用；运行中点回当前生效模型则撤销暂存。
 *
 * @param running 当前会话是否有运行中的 turn
 * @param current 当前生效的模型选择
 * @param next 用户点选的模型
 * @returns 立即应用、暂存或撤销暂存
 */
export function resolveModelSelect(
  running: boolean,
  current: RunModelSelection | null,
  next: RunModelSelection
): ModelSelectAction {
  if (!running) return { kind: "apply", selection: next };
  if (isSameModelSelection(current, next)) return { kind: "unstage" };
  return { kind: "stage", selection: next };
}
