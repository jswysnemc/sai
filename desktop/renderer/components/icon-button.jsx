import * as Toolbar from '@radix-ui/react-toolbar';
import * as Tooltip from '@radix-ui/react-tooltip';

/**
 * 【桌面界面】【图标按钮】统一键盘交互、可访问名称及栏内提示
 * @param {object} props 图标、标签、事件、禁用状态与样式
 * @returns {React.ReactElement} 标题栏按钮
 */
export function IconButton({ icon: Icon, label, onClick, disabled = false, danger = false, className = '' }) {
  return (
    <Tooltip.Root>
      <Tooltip.Trigger asChild>
        <Toolbar.Button aria-label={label} disabled={disabled} onClick={onClick}
          className={`icon-button ${danger ? 'icon-button-danger' : ''} ${className}`}>
          <Icon size={14} strokeWidth={1.5} aria-hidden="true" />
        </Toolbar.Button>
      </Tooltip.Trigger>
      <Tooltip.Portal>
        <Tooltip.Content side="left" sideOffset={8} className="toolbar-tooltip" avoidCollisions={false}>
          {label}
        </Tooltip.Content>
      </Tooltip.Portal>
    </Tooltip.Root>
  );
}
