import { ContextActionMenu } from "../../shared/ui/menu/context-action-menu";
import { stripExtendedPathPrefix } from "../workspace/workspace-path-utils";
import { useI18n } from "../i18n/use-i18n";

type WorkspaceContextMenuProps = {
  name: string;
  path: string;
  x: number;
  y: number;
  expanded: boolean;
  canClose: boolean;
  createPending: boolean;
  onClose: () => void;
  onCreateSession: () => void;
  onToggleExpanded: () => void;
  onCloseWorkspace: () => void;
};

/**
 * 渲染工作区右键菜单。
 *
 * 与会话菜单共用浮层，避免行内弹出层挡住时间和会话数。
 *
 * @param props 工作区名称、路径、坐标和操作回调
 * @returns 固定定位的操作菜单
 */
export function WorkspaceContextMenu({
  name,
  path,
  x,
  y,
  expanded,
  canClose,
  createPending,
  onClose,
  onCreateSession,
  onToggleExpanded,
  onCloseWorkspace
}: WorkspaceContextMenuProps) {
  const { t } = useI18n();
  return (
    <ContextActionMenu
      label={t(`Workspace actions for ${name}`, `${name} 的工作区操作`)}
      x={x}
      y={y}
      onClose={onClose}
      items={[
        {
          id: "create",
          label: t("New session", "新建会话"),
          disabled: createPending,
          onSelect: onCreateSession
        },
        {
          id: "expand",
          label: expanded ? t("Collapse sessions", "收起会话") : t("Expand sessions", "展开会话"),
          onSelect: onToggleExpanded
        },
        {
          id: "copy-path",
          label: t("Copy workspace path", "复制工作区路径"),
          onSelect: () => void navigator.clipboard.writeText(stripExtendedPathPrefix(path))
        },
        ...(canClose
          ? [{
              id: "close",
              label: t("Close workspace", "关闭工作区"),
              danger: true,
              separator: true,
              onSelect: onCloseWorkspace
            }]
          : [])
      ]}
    />
  );
}
