/** 模型切换标记：在对应条目前展示"from → to"分割线。 */
export type ModelSwitchMarker = {
  from: string;
  to: string;
};

/** 参与模型切换派生的时间线条目。 */
export type ModelSwitchEntry = {
  /** 条目的稳定标识（历史轮次为 turn_id，实时运行为 runId） */
  key: string;
  /** 该条目使用的模型；历史数据未记录时为空 */
  model?: string | null;
};

/**
 * 按时间顺序对比相邻条目的模型，派生每个条目前的模型切换标记。
 *
 * 未记录模型的条目（旧历史、尚未上报的实时运行）不参与对比，
 * 也不会中断前后条目之间的比较，保证旧数据混排时不误报切换。
 *
 * @param entries 按对话顺序排列的时间线条目
 * @returns 条目标识到切换标记的映射；模型未变化的条目不在其中
 */
export function deriveModelSwitchMarkers(
  entries: ModelSwitchEntry[]
): Map<string, ModelSwitchMarker> {
  const markers = new Map<string, ModelSwitchMarker>();
  let lastModel: string | null = null;
  for (const entry of entries) {
    const model = entry.model?.trim();
    if (!model) continue;
    if (lastModel && lastModel !== model) {
      markers.set(entry.key, { from: lastModel, to: model });
    }
    lastModel = model;
  }
  return markers;
}
