import { GitCommitHorizontal } from "lucide-react";
import type { GitCommitSummary } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import type { GitGraphPoint, GitGraphRowLayout } from "./graph-lanes";
import { formatGitDate, formatGitReference } from "./graph-utils";

const LANE_SPACING = 12;
const LANE_PADDING = 8;
const GRAPH_ROW_HEIGHT = 56;

type CommitGraphRowProps = {
  commit: GitCommitSummary;
  layout: GitGraphRowLayout;
  active: boolean;
  locale: string;
  onSelect: () => void;
  onContextMenu: (event: React.MouseEvent) => void;
};

/**
 * 渲染提交图中的单个提交、拓扑轨道和同步方向。
 *
 * @param props 提交数据、轨道布局、选择状态和交互回调
 * @returns 单行提交按钮
 */
export function CommitGraphRow(props: CommitGraphRowProps) {
  const { t } = useI18n();
  return (
    <Button
      className={`git-graph-row${props.active ? " active" : ""}`}
      onClick={props.onSelect}
      onContextMenu={props.onContextMenu}
    >
      <CommitGraphLane layout={props.layout} merge={props.commit.parents.length > 1} />
      <span className="git-graph-content">
        <strong>{props.commit.subject || props.commit.short_sha}</strong>
        <small>{props.commit.short_sha} · {props.commit.author_name} · {formatGitDate(props.commit.author_date, props.locale)}</small>
        {props.commit.refs.length > 0 && (
          <span className="git-graph-refs">
            {props.commit.refs.slice(0, 4).map((reference) => <em key={reference}>{formatGitReference(reference)}</em>)}
          </span>
        )}
      </span>
      <span className="git-graph-direction">
        {props.commit.local_only && <b>{t("Outgoing", "待推送")}</b>}
        {props.commit.remote_only && <i>{t("Incoming", "待拉取")}</i>}
        {props.commit.parents.length > 1 && <GitCommitHorizontal size={12} />}
      </span>
    </Button>
  );
}

/**
 * 使用 SVG 渲染单行提交的跨轨道连线和节点。
 *
 * @param props 轨道布局和是否为合并提交
 * @returns 提交轨道图形
 */
function CommitGraphLane({ layout, merge }: { layout: GitGraphRowLayout; merge: boolean }) {
  const width = LANE_PADDING * 2 + Math.max(0, layout.laneCount - 1) * LANE_SPACING;
  const nodeX = laneX(layout.nodeLane);
  const nodeY = GRAPH_ROW_HEIGHT / 2;
  return (
    <span className="git-graph-lanes" style={{ width }} aria-hidden="true">
      <svg viewBox={`0 0 ${width} ${GRAPH_ROW_HEIGHT}`} preserveAspectRatio="none">
        {layout.segments.map((segment, index) => {
          const start = pointPosition(segment.fromLane, segment.fromPoint);
          const end = pointPosition(segment.toLane, segment.toPoint);
          return (
            <path
              key={`${segment.fromLane}:${segment.fromPoint}:${segment.toLane}:${segment.toPoint}:${index}`}
              className={`git-lane-color-${segment.color}`}
              d={segmentPath(start.x, start.y, end.x, end.y)}
            />
          );
        })}
        <circle
          className={`git-graph-node git-lane-color-${layout.nodeColor}${merge ? " merge" : ""}`}
          cx={nodeX}
          cy={nodeY}
          r={merge ? 4.5 : 4}
        />
      </svg>
    </span>
  );
}

/**
 * 将轨道编号和纵向锚点转换为 SVG 坐标。
 *
 * @param lane 轨道编号
 * @param point 行顶部、节点或行底部
 * @returns SVG 坐标
 */
function pointPosition(lane: number, point: GitGraphPoint): { x: number; y: number } {
  const y = point === "top" ? 0 : point === "bottom" ? GRAPH_ROW_HEIGHT : GRAPH_ROW_HEIGHT / 2;
  return { x: laneX(lane), y };
}

/**
 * 计算指定轨道的横坐标。
 *
 * @param lane 轨道编号
 * @returns SVG 横坐标
 */
function laneX(lane: number): number {
  return LANE_PADDING + lane * LANE_SPACING;
}

/**
 * 创建平滑连接两个轨道锚点的 SVG 路径。
 *
 * @param startX 起点横坐标
 * @param startY 起点纵坐标
 * @param endX 终点横坐标
 * @param endY 终点纵坐标
 * @returns SVG path 数据
 */
function segmentPath(startX: number, startY: number, endX: number, endY: number): string {
  if (startX === endX) return `M ${startX} ${startY} L ${endX} ${endY}`;
  const middleY = (startY + endY) / 2;
  return `M ${startX} ${startY} C ${startX} ${middleY}, ${endX} ${middleY}, ${endX} ${endY}`;
}
