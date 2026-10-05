import * as Toolbar from '@radix-ui/react-toolbar';
import * as Tooltip from '@radix-ui/react-tooltip';
import { ArrowLeft, ArrowRight, Globe2, CircleHelp, SquareTerminal, Minus, Square, Copy, X, RotateCw } from 'lucide-react';
import { IconButton } from '../components/icon-button.jsx';
import { useDesktopState } from './use-desktop-state.jsx';
import mark from '../../build/icon.svg';

/**
 * 【桌面界面】【标题栏】呈现紧凑窗口控件，业务页面独立位于下方
 * @returns {React.ReactElement} 可拖动的自定义标题栏
 */
export function Titlebar() {
  const state = useDesktopState();
  const action = (name) => () => { void window.saiDesktop.perform(name); };
  return (
    <Tooltip.Provider delayDuration={600}>
      <Toolbar.Root className="titlebar flex h-10 items-center gap-1 px-2 sm:px-3" aria-label="桌面工具栏">
        <img src={mark} alt="Sai" className="app-mark mr-1 size-[1.125rem]" draggable="false" />
        <IconButton icon={ArrowLeft} label="后退" onClick={action('back')} disabled={!state.canGoBack} />
        <IconButton icon={ArrowRight} label="前进" onClick={action('forward')} disabled={!state.canGoForward} />
        <IconButton icon={RotateCw} label="刷新工作台" onClick={action('reload')} className="hidden sm:inline-flex" />
        <div className="drag-space min-w-4 flex-1 self-stretch" onDoubleClick={action('maximize')} aria-hidden="true" />
        <span className="mr-2 hidden max-w-72 truncate text-[0.75rem] text-[var(--muted)] lg:block">{state.title}</span>
        <IconButton icon={CircleHelp} label="使用说明" onClick={action('help')} className="hidden sm:inline-flex" />
        <IconButton icon={SquareTerminal} label="打开终端" onClick={action('terminal')} />
        <IconButton icon={Globe2} label="打开内置浏览器" onClick={action('browser')} />
        <Toolbar.Separator className="mx-1 h-3 w-px bg-[var(--line)]" />
        <IconButton icon={Minus} label="最小化" onClick={action('minimize')} />
        <IconButton icon={state.maximized ? Copy : Square} label={state.maximized ? '还原窗口' : '最大化'} onClick={action('maximize')} />
        <IconButton icon={X} label="关闭窗口" onClick={action('close')} danger />
      </Toolbar.Root>
    </Tooltip.Provider>
  );
}
