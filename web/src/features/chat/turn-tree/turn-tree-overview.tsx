import {
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type PointerEvent
} from "react";
import { Maximize, Minus, Plus } from "lucide-react";
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

/** 画布视图状态：平移偏移 + 缩放倍率。 */
type ViewState = {
  x: number;
  y: number;
  scale: number;
};

const COLUMN_WIDTH = 300;
const ROW_HEIGHT = 76;
const NODE_WIDTH = 276;
const NODE_HEIGHT = 44;
const MIN_SCALE = 0.3;
const MAX_SCALE = 2.5;
/** 适应窗口时四周保留的边距。 */
const FIT_PADDING = 24;

/**
 * 以节点图的形式总览整棵会话树，支持拖动平移与滚轮 / 按钮缩放。
 *
 * 侧边面板按缩进逐行列出，适合快速跳转；总览把分支画成折线连接的节点图，
 * 一眼能看出在哪里分叉、每条分支走了多远。
 *
 * @param props 树数据、忙碌状态与选择回调
 * @returns 树总览画布
 */
export function TurnTreeOverview({ tree, busy, onSelect }: TurnTreeOverviewProps) {
  const { t } = useI18n();
  const containerRef = useRef<HTMLDivElement | null>(null);
  const [view, setView] = useState<ViewState>({ x: 0, y: 0, scale: 1 });
  const viewRef = useRef(view);
  viewRef.current = view;
  const [dragging, setDragging] = useState(false);
  const dragRef = useRef<{ pointerId: number; startX: number; startY: number; originX: number; originY: number } | null>(null);
  const nodes = useMemo(() => placeNodes(tree), [tree]);
  const positions = useMemo(
    () => new Map(nodes.map((node) => [node.turnId, node])),
    [nodes]
  );

  const columns = nodes.length > 0 ? Math.max(...nodes.map((node) => node.column)) + 1 : 1;
  const rows = nodes.length > 0 ? Math.max(...nodes.map((node) => node.row)) + 1 : 1;
  const width = columns * COLUMN_WIDTH;
  const height = rows * ROW_HEIGHT;

  /** 计算并应用「适应窗口」视图：整棵树居中完整可见。 */
  const fitToViewport = () => {
    const container = containerRef.current;
    if (!container) return;
    const rect = container.getBoundingClientRect();
    const available = {
      width: Math.max(rect.width - FIT_PADDING * 2, 80),
      height: Math.max(rect.height - FIT_PADDING * 2, 80)
    };
    const scale = clampScale(Math.min(available.width / width, available.height / height, 1));
    setView({
      x: (rect.width - width * scale) / 2,
      y: (rect.height - height * scale) / 2,
      scale
    });
  };

  // 打开总览时自动适应窗口：大树先看全貌，再放大到感兴趣的分叉
  useLayoutEffect(() => {
    fitToViewport();
    // 仅在挂载与树规模变化时执行
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [width, height]);

  // 滚轮缩放以指针为锚点。React 的 onWheel 无法可靠 preventDefault，
  // 这里直接以非被动监听挂到容器上，阻止弹层随滚轮滚动
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    const handleWheel = (event: WheelEvent) => {
      event.preventDefault();
      const rect = container.getBoundingClientRect();
      const pointer = { x: event.clientX - rect.left, y: event.clientY - rect.top };
      const current = viewRef.current;
      const factor = Math.exp(-event.deltaY * 0.0015);
      const scale = clampScale(current.scale * factor);
      if (scale === current.scale) return;
      // 锚点在画布坐标系中的位置保持不变
      const anchor = {
        x: (pointer.x - current.x) / current.scale,
        y: (pointer.y - current.y) / current.scale
      };
      setView({
        x: pointer.x - anchor.x * scale,
        y: pointer.y - anchor.y * scale,
        scale
      });
    };
    container.addEventListener("wheel", handleWheel, { passive: false });
    return () => container.removeEventListener("wheel", handleWheel);
  }, []);

  if (nodes.length === 0) {
    return <p className="turn-tree-overview-empty">{t("No turns yet.", "还没有任何对话轮次。")}</p>;
  }

  /** 以视口中心为锚点按倍率缩放（缩放按钮使用）。 */
  const zoomBy = (factor: number) => {
    const container = containerRef.current;
    if (!container) return;
    const rect = container.getBoundingClientRect();
    const pointer = { x: rect.width / 2, y: rect.height / 2 };
    const current = viewRef.current;
    const scale = clampScale(current.scale * factor);
    if (scale === current.scale) return;
    const anchor = {
      x: (pointer.x - current.x) / current.scale,
      y: (pointer.y - current.y) / current.scale
    };
    setView({
      x: pointer.x - anchor.x * scale,
      y: pointer.y - anchor.y * scale,
      scale
    });
  };

  /** 在总览画布上按住空白处拖动，查看超出视口的分支。 */
  const handlePointerDown = (event: PointerEvent<HTMLDivElement>) => {
    if ((event.target as HTMLElement).closest("button")) return;
    // 阻止浏览器在拖动过程中选中节点文字
    event.preventDefault();
    window.getSelection()?.removeAllRanges();
    dragRef.current = {
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      originX: view.x,
      originY: view.y
    };
    event.currentTarget.setPointerCapture(event.pointerId);
    setDragging(true);
  };

  /** 更新拖动画布偏移量。 */
  const handlePointerMove = (event: PointerEvent<HTMLDivElement>) => {
    const drag = dragRef.current;
    if (!drag || drag.pointerId !== event.pointerId) return;
    setView((current) => ({
      ...current,
      x: drag.originX + event.clientX - drag.startX,
      y: drag.originY + event.clientY - drag.startY
    }));
  };

  /** 结束画布拖动。 */
  const handlePointerUp = (event: PointerEvent<HTMLDivElement>) => {
    if (!dragRef.current || dragRef.current.pointerId !== event.pointerId) return;
    dragRef.current = null;
    setDragging(false);
    event.currentTarget.releasePointerCapture(event.pointerId);
  };

  return (
    <div
      ref={containerRef}
      className={`turn-tree-overview${dragging ? " is-dragging" : ""}`}
      role="group"
      aria-label={t("Branch overview", "分支总览")}
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={handlePointerUp}
      onPointerCancel={handlePointerUp}
    >
      {/* 节点与连线都相对本层绝对定位，因此这层必须写死真实画布尺寸 */}
      <div
        className="turn-tree-overview-stage"
        style={{
          width,
          height,
          transform: `translate(${view.x}px, ${view.y}px) scale(${view.scale})`
        }}
      >
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
      <div className="turn-tree-overview-controls" role="group" aria-label={t("Zoom", "缩放")}>
        <button
          type="button"
          onClick={() => zoomBy(1 / 1.25)}
          title={t("Zoom out", "缩小")}
          aria-label={t("Zoom out", "缩小")}
        >
          <Minus size={13} />
        </button>
        <button
          type="button"
          className="turn-tree-overview-zoom-level"
          onClick={fitToViewport}
          title={t("Fit to view", "适应窗口")}
        >
          {Math.round(view.scale * 100)}%
        </button>
        <button
          type="button"
          onClick={() => zoomBy(1.25)}
          title={t("Zoom in", "放大")}
          aria-label={t("Zoom in", "放大")}
        >
          <Plus size={13} />
        </button>
        <button
          type="button"
          onClick={fitToViewport}
          title={t("Fit to view", "适应窗口")}
          aria-label={t("Fit to view", "适应窗口")}
        >
          <Maximize size={13} />
        </button>
      </div>
    </div>
  );
}

/**
 * 把倍率限制在可用范围内。
 *
 * @param scale 待限制的倍率
 * @returns 合法倍率
 */
function clampScale(scale: number): number {
  return Math.min(Math.max(scale, MIN_SCALE), MAX_SCALE);
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
