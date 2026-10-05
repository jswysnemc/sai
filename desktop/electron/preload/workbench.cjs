const { ipcRenderer } = require('electron');

/**
 * 【桌面适配】【主题同步】只上报明确的配色变量，不向业务页面暴露窗口操作接口
 * @returns {void} 无返回值
 */
function reportAppearance() {
  const style = getComputedStyle(document.documentElement);
  const read = (name) => style.getPropertyValue(name).trim();
  ipcRenderer.send('desktop:appearance', {
    surface: read('--sidebar-surface'), paper: read('--paper'), ink: read('--ink'),
    muted: read('--ink-soft'), line: read('--line'),
  });
}

if (process.isMainFrame) {
  window.addEventListener('DOMContentLoaded', () => {
    reportAppearance();
    const observer = new MutationObserver(reportAppearance);
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme', 'class', 'style'] });
    window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', reportAppearance);
    window.addEventListener('load', reportAppearance, { once: true });
  });
  ipcRenderer.on('desktop:open-panel', (_event, tab) => {
    if (tab === 'browser' || tab === 'terminal') {
      window.dispatchEvent(new CustomEvent('sai:open-workspace-panel', { detail: { tab } }));
    }
  });
}
