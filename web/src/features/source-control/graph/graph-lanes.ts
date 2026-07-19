import type { GitCommitSummary } from "../../../api/contracts";

export type GitGraphPoint = "top" | "node" | "bottom";

export type GitGraphSegment = {
  fromLane: number;
  fromPoint: GitGraphPoint;
  toLane: number;
  toPoint: GitGraphPoint;
  color: number;
};

export type GitGraphRowLayout = {
  nodeLane: number;
  nodeColor: number;
  laneCount: number;
  segments: GitGraphSegment[];
};

type ActiveLane = {
  sha: string;
  color: number;
};

const GRAPH_LANE_COLOR_COUNT = 6;

/**
 * 根据拓扑排序提交的父节点关系计算每行轨道布局。
 *
 * @param commits 按新到旧排列的提交摘要
 * @returns 与提交索引一一对应的节点、轨道数和连线
 */
export function calculateGitGraphLanes(commits: GitCommitSummary[]): GitGraphRowLayout[] {
  let active: ActiveLane[] = [];
  let nextColor = 0;
  return commits.map((commit) => {
    const top = active.map((lane) => ({ ...lane }));
    const existingLane = top.findIndex((lane) => lane.sha === commit.sha);
    const nodeLane = existingLane >= 0 ? existingLane : top.length;
    const nodeColor = existingLane >= 0 ? top[existingLane].color : nextColor++ % GRAPH_LANE_COLOR_COUNT;
    const remaining = top.filter((_, index) => index !== existingLane);

    // 1. 第一父节点延续当前颜色，其余新父节点分配独立颜色
    let insertionIndex = Math.min(nodeLane, remaining.length);
    for (const [parentIndex, parent] of uniqueParents(commit.parents).entries()) {
      const existingParent = remaining.findIndex((lane) => lane.sha === parent);
      if (existingParent >= 0) {
        insertionIndex = existingParent + 1;
        continue;
      }
      const color = parentIndex === 0 ? nodeColor : nextColor++ % GRAPH_LANE_COLOR_COUNT;
      remaining.splice(insertionIndex, 0, { sha: parent, color });
      insertionIndex += 1;
    }
    active = remaining;

    // 2. 未消费轨道跨过当前行，当前提交连接顶部来源和全部父节点
    const segments: GitGraphSegment[] = [];
    for (const [topLane, lane] of top.entries()) {
      if (topLane === existingLane) continue;
      const bottomLane = active.findIndex((candidate) => candidate.sha === lane.sha);
      if (bottomLane >= 0) {
        segments.push({
          fromLane: topLane,
          fromPoint: "top",
          toLane: bottomLane,
          toPoint: "bottom",
          color: lane.color
        });
      }
    }
    if (existingLane >= 0) {
      segments.push({
        fromLane: existingLane,
        fromPoint: "top",
        toLane: nodeLane,
        toPoint: "node",
        color: nodeColor
      });
    }
    for (const parent of uniqueParents(commit.parents)) {
      const parentLane = active.findIndex((lane) => lane.sha === parent);
      if (parentLane >= 0) {
        segments.push({
          fromLane: nodeLane,
          fromPoint: "node",
          toLane: parentLane,
          toPoint: "bottom",
          color: active[parentLane].color
        });
      }
    }
    const laneCount = Math.max(1, top.length, active.length, nodeLane + 1);
    return { nodeLane, nodeColor, laneCount, segments };
  });
}

/**
 * 去除异常提交数据中的重复或空父节点，并保留 Git 父节点顺序。
 *
 * @param parents 原始父提交列表
 * @returns 去重后的父提交列表
 */
function uniqueParents(parents: string[]): string[] {
  return [...new Set(parents.map((parent) => parent.trim()).filter(Boolean))];
}
