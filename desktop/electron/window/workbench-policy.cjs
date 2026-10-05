const { shell } = require('electron');
const { isInternal, isExternal } = require('../navigation.cjs');

/**
 * 【桌面适配】【工作台隔离】仅为 Sai 页面授予有限权限，并分流外部导航
 * @param {WebContents} contents 工作台内容
 * @param {URL} url 当前后端地址
 * @param {object} logger 日志接口
 * @returns {Promise<void>} 会话代理设置完成后返回
 */
async function configureWorkbench(contents, url, logger) {
  const partition = contents.session;
  await partition.setProxy({ mode: 'system', proxyBypassRules: '<local>;127.0.0.1;[::1]' });
  const allowed = new Set(['clipboard-read', 'clipboard-sanitized-write', 'fullscreen']);
  partition.setPermissionCheckHandler((page, permission, origin) =>
    Boolean(page === contents && isInternal(page.getURL(), url.origin) && isInternal(origin, url.origin) && allowed.has(permission)));
  partition.setPermissionRequestHandler((page, permission, callback, details) =>
    callback(Boolean(page === contents && isInternal(page.getURL(), url.origin)
      && isInternal(details.requestingUrl, url.origin) && allowed.has(permission))));
  partition.setDevicePermissionHandler(() => false);

  /**
   * 【桌面适配】【链接分流】同源页面留在工作台，其他网页交给系统浏览器
   * @param {string} target 用户请求的链接
   * @returns {void} 无返回值
   */
  function openLink(target) {
    const operation = isInternal(target, url.origin) ? contents.loadURL(target)
      : isExternal(target) ? shell.openExternal(target) : null;
    operation?.catch((error) => logger.error(`【桌面端】【链接打开】${error.message}`));
  }
  contents.setWindowOpenHandler(({ url: target }) => { openLink(target); return { action: 'deny' }; });
  contents.on('will-navigate', (event, target) => {
    if (!isInternal(target, url.origin)) { event.preventDefault(); openLink(target); }
  });
  contents.on('will-redirect', (event, target) => { if (!isInternal(target, url.origin)) event.preventDefault(); });
  contents.on('will-attach-webview', (event) => event.preventDefault());
  contents.on('render-process-gone', (_event, details) => logger.error(`【桌面端】【页面进程退出】${details.reason}`));
}

module.exports = { configureWorkbench };
