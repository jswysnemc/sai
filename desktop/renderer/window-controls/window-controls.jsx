import { Minus, Square, Copy, X } from 'lucide-react';
import { useDesktopState } from './use-desktop-state.jsx';

/**
 * 【桌面界面】【窗口按钮】叠在工作台顶行右端的最小化、最大化与关闭按钮
 * @returns {React.ReactElement} 与工作台顶行等高的按钮组
 */
export function WindowControls() {
  const state = useDesktopState();
  const action = (name) => () => { void window.saiDesktop.perform(name); };
  const maximizeLabel = state.maximized ? '还原窗口' : '最大化';
  return (
    <div className="window-controls" role="group" aria-label="窗口控制">
      <button type="button" className="caption-button" aria-label="最小化" title="最小化" onClick={action('minimize')}>
        <Minus size={14} strokeWidth={1.25} aria-hidden="true" />
      </button>
      <button type="button" className="caption-button" aria-label={maximizeLabel} title={maximizeLabel} onClick={action('maximize')}>
        {state.maximized
          ? <Copy size={12} strokeWidth={1.25} aria-hidden="true" />
          : <Square size={12} strokeWidth={1.25} aria-hidden="true" />}
      </button>
      <button type="button" className="caption-button caption-button-close" aria-label="关闭窗口" title="关闭窗口" onClick={action('close')}>
        <X size={15} strokeWidth={1.25} aria-hidden="true" />
      </button>
    </div>
  );
}
