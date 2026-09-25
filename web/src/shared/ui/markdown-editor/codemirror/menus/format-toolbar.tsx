import type { EditorView } from "@codemirror/view";
import type { FormatAction } from "../editor-format-actions";

type FormatToolbarProps = {
  view: EditorView;
  /** 分组展示的动作，组间以细分隔线隔开 */
  groups: FormatAction[][];
  /** 执行动作后回调，如关闭菜单 */
  onDone?: () => void;
};

/**
 * 渲染紧凑的格式图标条，供右键菜单顶部与选中气泡复用。
 *
 * 按钮在 mousedown 时阻止默认行为，点击不会让编辑器失焦、选区保持不变。
 *
 * @param props 编辑器视图、动作分组与完成回调
 * @returns 图标按钮条
 */
export function FormatToolbar({ view, groups, onDone }: FormatToolbarProps) {
  return (
    <div className="md-format-toolbar" role="toolbar">
      {groups.map((group, index) => (
        <div key={index} className="md-format-group">
          {group.map((action) => {
            const Icon = action.icon;
            const active = action.active(view.state);
            const title = action.shortcut ? `${action.label} (${action.shortcut})` : action.label;
            return (
              <button
                key={action.id}
                type="button"
                className={active ? "is-active" : undefined}
                aria-pressed={active}
                aria-label={action.label}
                title={title}
                onMouseDown={(event) => event.preventDefault()}
                onClick={() => {
                  action.run(view);
                  view.focus();
                  onDone?.();
                }}
              >
                <Icon size={14} />
              </button>
            );
          })}
        </div>
      ))}
    </div>
  );
}
