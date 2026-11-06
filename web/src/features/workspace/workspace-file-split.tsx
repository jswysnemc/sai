import {
  useEffect,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent,
  type PointerEvent as ReactPointerEvent,
  type ReactNode
} from "react";
import { useI18n } from "../i18n/use-i18n";
import {
  WORKSPACE_EDITOR_MIN_WIDTH,
  WORKSPACE_FILE_TREE_DEFAULT_WIDTH,
  WORKSPACE_FILE_TREE_MIN_WIDTH,
  shouldOverlayWorkspaceFileTree,
  useWorkspaceFileSplitState
} from "./workspace-file-split-state";

type WorkspaceFileSplitProps = {
  open: boolean;
  editor: ReactNode;
  tree: ReactNode;
  onOverlayChange: (overlay: boolean) => void;
};

/**
 * 渲染可调整宽度的编辑器与文件树双栏布局。
 *
 * 宽度不足时自动回退为覆盖式文件树，避免两侧同时缩至不可用。
 *
 * @param props 展开状态、两侧内容和覆盖状态回调
 * @returns 编辑器与文件树分栏
 */
export function WorkspaceFileSplit(props: WorkspaceFileSplitProps) {
  const { t } = useI18n();
  const rootRef = useRef<HTMLDivElement>(null);
  const cleanupDragRef = useRef<(() => void) | null>(null);
  const [overlay, setOverlay] = useState(false);
  const [containerWidth, setContainerWidth] = useState(0);
  const { treeWidth, resize, constrain } = useWorkspaceFileSplitState();

  useEffect(() => {
    return () => cleanupDragRef.current?.();
  }, [props.open, overlay]);

  useEffect(() => {
    const root = rootRef.current;
    if (!root || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(([entry]) => {
      const width = entry.contentRect.width;
      setContainerWidth(width);
      const nextOverlay = shouldOverlayWorkspaceFileTree(width);
      setOverlay(nextOverlay);
      if (!nextOverlay) constrain(width);
    });
    observer.observe(root);
    return () => observer.disconnect();
  }, [constrain]);

  useEffect(() => {
    props.onOverlayChange(overlay);
  }, [overlay, props.onOverlayChange]);

  /**
   * 开始拖动分隔条，并在指针释放后清理全局监听。
   *
   * @param event 分隔条指针按下事件
   * @returns 无
   */
  const handlePointerDown = (event: ReactPointerEvent<HTMLDivElement>): void => {
    if (overlay || event.button !== 0) return;
    event.preventDefault();
    const bounds = rootRef.current?.getBoundingClientRect();
    if (!bounds) return;
    cleanupDragRef.current?.();
    document.body.classList.add("workspace-file-split-resizing");

    // 1. 【Web 工作台】【文件树拖动】按指针到右缘的距离计算文件树宽度
    /** 更新文件树宽度；moveEvent 为指针移动事件，无返回值 */
    const handlePointerMove = (moveEvent: PointerEvent) => {
      resize(bounds.right - moveEvent.clientX, bounds.width);
    };

    // 2. 【Web 工作台】【文件树拖动】释放、取消或卸载时清理监听和拖动态
    /** 清理本次拖动的全局监听，无参数和返回值 */
    const handlePointerUp = () => {
      document.body.classList.remove("workspace-file-split-resizing");
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", handlePointerUp);
      window.removeEventListener("pointercancel", handlePointerUp);
      cleanupDragRef.current = null;
    };

    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", handlePointerUp, { once: true });
    window.addEventListener("pointercancel", handlePointerUp, { once: true });
    cleanupDragRef.current = handlePointerUp;
  };

  /**
   * 使用方向键、Home 和 End 调整文件树宽度。
   *
   * @param event 分隔条键盘事件
   * @returns 无
   */
  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>): void => {
    const currentWidth = containerWidth || rootRef.current?.getBoundingClientRect().width || window.innerWidth;
    const maximum = Math.max(
      WORKSPACE_FILE_TREE_MIN_WIDTH,
      currentWidth - WORKSPACE_EDITOR_MIN_WIDTH
    );
    const widths: Partial<Record<string, number>> = {
      ArrowLeft: treeWidth + 16,
      ArrowRight: treeWidth - 16,
      Home: WORKSPACE_FILE_TREE_MIN_WIDTH,
      End: maximum
    };
    const nextWidth = widths[event.key];
    if (nextWidth === undefined) return;
    event.preventDefault();
    resize(nextWidth, currentWidth);
  };

  const style = { "--workspace-file-tree-width": `${treeWidth}px` } as CSSProperties;
  const maximum = Math.max(
    WORKSPACE_FILE_TREE_MIN_WIDTH,
    (containerWidth || rootRef.current?.clientWidth || 0) - WORKSPACE_EDITOR_MIN_WIDTH
  );
  const classes = [
    "files-layout",
    props.open ? "file-tree-open" : "file-tree-closed",
    overlay ? "file-tree-overlay" : ""
  ].filter(Boolean).join(" ");

  return (
    <div ref={rootRef} className={classes} style={style}>
      {props.editor}
      {props.open && !overlay && (
        <div
          className="workspace-file-split-handle"
          role="separator"
          tabIndex={0}
          aria-label={t("Resize editor and file tree", "调整编辑器与文件树宽度")}
          aria-orientation="vertical"
          aria-valuemin={WORKSPACE_FILE_TREE_MIN_WIDTH}
          aria-valuemax={Math.round(maximum)}
          aria-valuenow={Math.round(treeWidth)}
          onPointerDown={handlePointerDown}
          onDoubleClick={() => resize(
            WORKSPACE_FILE_TREE_DEFAULT_WIDTH,
            rootRef.current?.clientWidth ?? window.innerWidth
          )}
          onKeyDown={handleKeyDown}
        >
          <span />
        </div>
      )}
      {props.open && props.tree}
    </div>
  );
}
