const { WebSocket } = require('ws');
const { allowedPageUrl } = require('./browser-tabs.cjs');

/**
 * 【桌面浏览器】【协议连接】转发原生页面 CDP，仅适配 Electron 缺少的浏览器级命令
 * @param {object} options 客户端、原生端点、标签管理器、下载管理器和退出回调
 * @returns {void} 连接关闭时自动移除监听器
 */
function connectCdp({ client, upstreamUrl, tabs, downloads, quit, pageTarget }) {
  const upstream = new WebSocket(upstreamUrl, { maxPayload: 256 * 1024 * 1024 });
  const sessions = new Map();
  const pendingAttachments = new Map();
  const queue = [];
  let discover = false;
  const send = (value) => {
    if (client.readyState === WebSocket.OPEN) client.send(JSON.stringify(value));
  };
  const targetEvent = (method, params) => { if (discover) send({ method, params }); };
  const downloadEvent = (method, params) => send({ method, params });
  tabs.on('target', targetEvent);
  downloads.on('event', downloadEvent);

  /**
   * 【桌面浏览器】【命令适配】处理有差异的浏览器命令，其余原样交给 Chromium
   * @param {object} message 已解析的 CDP 请求
   * @returns {{handled: boolean, result?: object}} 是否已处理及返回值
   */
  function intercept(message) {
    const params = message.params || {};
    const target = pageTarget || sessions.get(message.sessionId);
    switch (message.method) {
      case 'Target.setDiscoverTargets':
        discover = Boolean(params.discover);
        if (discover) for (const id of tabs.tabs.keys()) targetEvent('Target.targetCreated', { targetInfo: tabs.info(id) });
        return { handled: true, result: {} };
      case 'Target.getTargets':
        return { handled: true, result: { targetInfos: [...tabs.tabs.keys()].map((id) => tabs.info(id)) } };
      case 'Target.createTarget':
        return { handled: true, result: { targetId: tabs.create(params.url || 'about:blank') } };
      case 'Target.closeTarget':
        return { handled: true, result: { success: tabs.close(params.targetId) } };
      case 'Target.activateTarget':
        tabs.info(params.targetId);
        return { handled: true, result: {} };
      case 'Target.attachToTarget':
        tabs.info(params.targetId);
        pendingAttachments.set(message.id, params.targetId);
        break;
      case 'Browser.setDownloadBehavior':
        downloads.configure(params);
        return { handled: true, result: {} };
      case 'Browser.close':
        setTimeout(quit, 100);
        return { handled: true, result: {} };
      case 'Page.bringToFront':
        // 【桌面浏览器】【离屏保持】面板切换标签时不能把离屏窗口显示到用户桌面
        return { handled: true, result: {} };
      case 'Page.navigate':
        if (!allowedPageUrl(params.url || '')) throw new Error('不允许的浏览器地址');
        break;
      case 'Emulation.setDeviceMetricsOverride':
        if (target) tabs.resize(target, params);
        break;
    }
    return { handled: false };
  }

  client.on('message', (raw) => {
    let message;
    try {
      message = JSON.parse(raw.toString());
      if (!Number.isSafeInteger(message.id) || typeof message.method !== 'string') throw new Error('无效的 CDP 请求');
      const response = intercept(message);
      if (response.handled) {
        send({ id: message.id, result: response.result, ...(message.sessionId ? { sessionId: message.sessionId } : {}) });
      } else if (upstream.readyState === WebSocket.OPEN) {
        upstream.send(raw.toString());
      } else if (queue.length < 128 && upstream.readyState === WebSocket.CONNECTING) {
        queue.push(raw.toString());
      } else {
        throw new Error('浏览器调试连接尚未就绪');
      }
    } catch (error) {
      send({ id: message?.id ?? 0, error: { code: -32602, message: error.message } });
    }
  });
  upstream.on('open', () => { for (const message of queue.splice(0)) upstream.send(message); });
  upstream.on('message', (raw) => {
    const message = JSON.parse(raw.toString());
    // 【桌面浏览器】【目标隔离】标签事件由管理器提供，排除开发工具等内部页面
    if (['Target.targetCreated', 'Target.targetInfoChanged', 'Target.targetDestroyed'].includes(message.method)) return;
    if (pendingAttachments.has(message.id)) {
      if (message.result?.sessionId) sessions.set(message.result.sessionId, pendingAttachments.get(message.id));
      pendingAttachments.delete(message.id);
    }
    if (client.readyState === WebSocket.OPEN) client.send(raw.toString());
  });
  upstream.on('error', () => client.close(1011, 'Chromium connection failed'));
  upstream.on('close', () => client.close());
  client.on('error', () => upstream.close());
  client.once('close', () => {
    upstream.close();
    tabs.off('target', targetEvent);
    downloads.off('event', downloadEvent);
  });
}

module.exports = { connectCdp };
