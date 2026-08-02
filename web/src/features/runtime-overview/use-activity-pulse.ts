import { useEffect, useRef, useState } from "react";
import { useI18n } from "../i18n/use-i18n";
import { detectActivityPulse, type ActivityPulse, type ActivitySnapshot } from "./activity-pulse";

/** 一条播报在总览上停留的时长 */
const PULSE_DURATION_MS = 3200;

/**
 * 监测运行状态变化，产出短暂的活动播报。
 *
 * 播报只停留数秒，随后自动回到常态，让总览在「安静展示 Git 改动」与
 * 「有事发生时说一声」之间自然切换。
 *
 * @param snapshot 当前运行状态快照
 * @returns 当前播报；无播报时为 null
 */
export function useActivityPulse(snapshot: ActivitySnapshot): ActivityPulse | null {
  const { t } = useI18n();
  const [pulse, setPulse] = useState<ActivityPulse | null>(null);
  const previousRef = useRef<ActivitySnapshot | null>(null);
  const timerRef = useRef<number | null>(null);
  // t 每次渲染都是新引用，放进 ref 以免把它写进依赖导致重复触发
  const translateRef = useRef(t);
  translateRef.current = t;

  const { runningTasks, runningSubagents, completedTodos, totalTodos } = snapshot;

  useEffect(() => {
    const current = { runningTasks, runningSubagents, completedTodos, totalTodos };
    const next = detectActivityPulse(previousRef.current, current, translateRef.current);
    previousRef.current = current;
    if (!next) return;
    setPulse(next);
    // 新播报覆盖旧播报，计时从头开始
    if (timerRef.current !== null) window.clearTimeout(timerRef.current);
    timerRef.current = window.setTimeout(() => setPulse(null), PULSE_DURATION_MS);
  }, [completedTodos, runningSubagents, runningTasks, totalTodos]);

  useEffect(() => () => {
    if (timerRef.current !== null) window.clearTimeout(timerRef.current);
  }, []);

  return pulse;
}
