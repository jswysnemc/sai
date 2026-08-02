/**
 * 运行总览的活动播报。
 *
 * 总览常态展示 Git 改动。但后台命令启停、Todo 推进、子智能体启停这些事件
 * 转瞬即逝，用户往往正看着别处。这里把它们检测出来，让总览临时切换到
 * 对应内容，过一会儿再回到常态，用一次短暂的内容变化传达"有事发生"。
 */

/** 可播报的活动类型。 */
export type ActivityKind = "task" | "todo" | "subagent";

/** 一条活动播报。 */
export type ActivityPulse = {
  kind: ActivityKind;
  /** 播报正文，已本地化 */
  message: string;
};

/** 参与比对的运行状态快照。 */
export type ActivitySnapshot = {
  /** 运行中的后台命令数 */
  runningTasks: number;
  /** 运行中的子智能体数 */
  runningSubagents: number;
  /** 已完成的 Todo 数 */
  completedTodos: number;
  /** Todo 总数 */
  totalTodos: number;
};

type Translate = (en: string, zh: string) => string;

/**
 * 比较前后两次快照，得出需要播报的活动。
 *
 * 只在数量真正变化时播报；同一时刻有多类变化时按「子智能体 > 后台命令 > Todo」
 * 取其一，避免播报相互覆盖导致谁都看不清。
 *
 * @param previous 上一次快照；首次渲染时为 null，此时不播报
 * @param current 当前快照
 * @param t 双语文本选择方法
 * @returns 需要播报的活动；无变化时返回 null
 */
export function detectActivityPulse(
  previous: ActivitySnapshot | null,
  current: ActivitySnapshot,
  t: Translate
): ActivityPulse | null {
  // 首次拿到数据时不播报，否则一进页面就会闪一下
  if (!previous) return null;

  // 1. 子智能体启停优先级最高：它通常代表一段较长的工作开始或结束
  if (current.runningSubagents > previous.runningSubagents) {
    return {
      kind: "subagent",
      message: t(
        `${current.runningSubagents} subagent(s) running`,
        `${current.runningSubagents} 个子智能体运行中`
      )
    };
  }
  if (current.runningSubagents < previous.runningSubagents) {
    return {
      kind: "subagent",
      message: current.runningSubagents > 0
        ? t(`${current.runningSubagents} subagent(s) still running`, `还有 ${current.runningSubagents} 个子智能体运行中`)
        : t("Subagents finished", "子智能体已结束")
    };
  }

  // 2. 后台命令启停
  if (current.runningTasks > previous.runningTasks) {
    return {
      kind: "task",
      message: t(
        `${current.runningTasks} background task(s) running`,
        `${current.runningTasks} 个后台任务运行中`
      )
    };
  }
  if (current.runningTasks < previous.runningTasks) {
    return {
      kind: "task",
      message: current.runningTasks > 0
        ? t(`${current.runningTasks} background task(s) still running`, `还有 ${current.runningTasks} 个后台任务运行中`)
        : t("Background tasks finished", "后台任务已结束")
    };
  }

  // 3. Todo 推进：完成数变化，或计划本身被重建
  if (current.completedTodos !== previous.completedTodos && current.totalTodos > 0) {
    return {
      kind: "todo",
      message: t(
        `Plan ${current.completedTodos}/${current.totalTodos}`,
        `计划推进 ${current.completedTodos}/${current.totalTodos}`
      )
    };
  }
  if (current.totalTodos !== previous.totalTodos && current.totalTodos > 0) {
    return {
      kind: "todo",
      message: t(`Plan updated · ${current.totalTodos} steps`, `计划已更新 · ${current.totalTodos} 步`)
    };
  }

  return null;
}
