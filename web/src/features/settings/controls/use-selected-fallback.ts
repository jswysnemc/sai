import { useEffect, useRef } from "react";

/**
 * 保证列表选中项始终有效：选中项从候选中消失时回落。
 *
 * 设置页多处「对象列表 + 编辑区」共用此行为——删除或重命名当前项后，
 * 编辑区自动切到回落候选，不会停留在悬空引用上。
 *
 * 参数:
 * - `selectedId`: 当前选中标识
 * - `ids`: 合法候选标识列表
 * - `onReset`: 回落时的选中更新回调
 * - `fallbackId`: 优先回落目标；缺省取候选首项
 *
 * 返回:
 * - 无
 */
export function useSelectedFallback(
  selectedId: string,
  ids: readonly string[],
  onReset: (id: string) => void,
  fallbackId = ""
): void {
  // 回调存 ref：它通常是 setState，不应该参与依赖比较
  const reset = useRef(onReset);
  reset.current = onReset;
  // 候选序列化为签名，调用方内联数组不会导致每次渲染重跑；
  // 分隔符取 NUL，避免 id 含空格时不同列表得到相同签名
  const signature = ids.join("\0");
  useEffect(() => {
    if (ids.includes(selectedId)) return;
    const next = fallbackId || ids[0] || "";
    if (next !== selectedId) reset.current(next);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedId, signature, fallbackId]);
}
