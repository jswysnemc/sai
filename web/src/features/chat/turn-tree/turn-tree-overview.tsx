import { useMemo } from "react";
import type { SessionTurnTree } from "../../../api/turn-tree-contracts";
import { useI18n } from "../../i18n/use-i18n";
import { flattenTurnTree } from "./turn-tree-rows";
import "./turn-tree-overview.css";

type TurnTreeOverviewProps = {
  tree: SessionTurnTree;
  busy?: boolean;
  onSelect: (turnId: string) => void;
};

/** 单个节点在画布上的位置。 */
type PlacedNode = {
  turnId: string;
  /** 列，对应树的深度 */
  column: number;
  /** 行，按压平顺序自上而下 */
  row: number;
  summary: string;
  status: string;
  isActive: boolean;
  onActivePath: boolean;
  parentTurnId: string | null;
};

const COLUMN_WIDTH = 168;
const ROW_HEIGHT = 44;
const NODE_WIDTH = 148;
const NODE_HEIGHT = 30;

/**
 * 以节点图的形式总览整棵会话树。
 *
 * 侧边面板按缩进逐行列出，适合快速跳转；总览把分支画成折线连接的节点图，
 * 一眼能看出在哪里分叉、每条分支走了多远。
 *
 * @param props 树数据、忙碌状态与选择回调
 * @returns 树总览画布
 */
export function TurnTreeOverview({ tree, busy, onSelect }: TurnTreeOverviewProps) {
  const { t } = useI18n();
  const nodes = useMemo(() => placeNodes(tree), [tree]);
  const positions = useMemo(
    () => new Map(nodes.map((node) => [node.turnId, node])),
    [nodes]
  );

  if (nodes.length === 0) {
    return <p className="turn-tree-overview-empty">{t("No turns yet.", "还没有任何对话轮次。")}</p>;
  }

  const columns = Math.max(...nodes.map((node) => node.column)) + 1;
  const rows = Math.max(...nodes.map((node) => node.row)) + 1;
  const width = columns * COLUMN_WIDTH;
  const height = rows * ROW_HEIGHT;

  return (
    <div className="turn-tree-overview" role="group" aria-label={t("Branch overview", "分支总览")}>
      <svg width={width} height={height} className="turn-tree-overview-canvas">
        {/* 先画连线，节点覆盖其上，避免折线压住文字 */}
        {nodes.map((node) => {
          if (!node.parentTurnId) return null;
          const parent = positions.get(node.parentTurnId);
          if (!parent) return null;
          return (
            <path
              key={`edge-${node.turnId}`}
              className={node.onActivePath ? "edge on-path" : "edge"}
              d={elbowPath(parent, node)}
              fill="none"
            />
          );
        })}
      </svg>
      {nodes.map((node) => (
        <button
          key={node.turnId}
          type="button"
          className={[
            "turn-tree-overview-node",
            node.isActive ? "is-active" : "",
            node.onActivePath ? "on-path" : "",
            `is-${node.status}`
          ]
            .filter(Boolean)
            .join(" ")}
          style={{
            left: node.column * COLUMN_WIDTH,
            top: node.row * ROW_HEIGHT,
            width: NODE_WIDTH,
            height: NODE_HEIGHT
          }}
          disabled={busy || node.isActive}
          onClick={() => onSelect(node.turnId)}
          title={node.summary}
        >
          <span>{node.summary || t("(empty)", "（空）")}</span>
        </button>
      ))}
    </div>
  );
}

/**
 * 计算每个节点在画布上的行列位置。
 *
 * 列取决于深度，行按压平顺序递增，因此同一条分支在视觉上向右下方延伸。
 *
 * @param tree 会话分支树
 * @returns 带坐标的节点列表
 */
function placeNodes(tree: SessionTurnTree): PlacedNode[] {
  return flattenTurnTree(tree).map((row, index) => ({
    turnId: row.node.turn_id,
    column: row.depth,
    row: index,
    summary: row.node.user_summary,
    status: row.node.status,
    isActive: row.isActive,
    onActivePath: row.onActivePath,
    parentTurnId: row.node.parent_turn_id
  }));
}

/**
 * 生成父子节点之间的折线路径。
 *
 * 先从父节点底部垂直向下，再水平折向子节点，形成清晰的层级感。
 *
 * @param parent 父节点位置
 * @param child 子节点位置
 * @returns SVG path 的 d 属性
 */
function elbowPath(parent: PlacedNode, child: PlacedNode): string {
  const startX = parent.column * COLUMN_WIDTH + 12;
  const startY = parent.row * ROW_HEIGHT + NODE_HEIGHT;
  const endX = child.column * COLUMN_WIDTH;
  const endY = child.row * ROW_HEIGHT + NODE_HEIGHT / 2;
  return `M ${startX} ${startY} V ${endY} H ${endX}`;
}
