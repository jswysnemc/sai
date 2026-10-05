const http = require('node:http');
const { randomUUID } = require('node:crypto');
const { WebSocketServer } = require('ws');
const { connectCdp } = require('./cdp-connection.cjs');

/**
 * 【桌面浏览器】【连接来源】只允许无 Origin 的本机协议客户端及同源调试页面
 * @param {string|undefined} origin 浏览器发送的 Origin
 * @param {number} port 当前协议服务端口
 * @returns {boolean} 是否允许连接
 */
function allowedOrigin(origin, port) {
  return origin === undefined || origin === `http://127.0.0.1:${port}`;
}

/**
 * 【桌面浏览器】【协议服务】提供兼容 Sai 的 DevTools 入口与原生调试页面代理
 * @param {object} options 原生 Chromium 地址、标签、下载及退出接口
 * @returns {Promise<object>} 端口、浏览器路径和关闭方法
 */
async function startCdpServer({ upstreamUrl, tabs, downloads, quit }) {
  const upstream = new URL(upstreamUrl);
  const browserPath = `/devtools/browser/${randomUUID()}`;
  const sockets = new WebSocketServer({ noServer: true, maxPayload: 16 * 1024 * 1024 });
  let port;
  let rootConnections = 0;
  const server = http.createServer((request, response) => {
    if (!allowedOrigin(request.headers.origin, port)) { response.writeHead(403).end(); return; }
    if (request.url === '/json/version') {
      response.setHeader('Content-Type', 'application/json');
      response.end(JSON.stringify({ Browser: `Chrome/${process.versions.chrome}`, 'Protocol-Version': '1.3',
        webSocketDebuggerUrl: `ws://127.0.0.1:${port}${browserPath}` }));
      return;
    }
    // 【桌面浏览器】【调试资源】只代理到同一进程的 Chromium，不接受任意上游地址
    if (!request.url?.startsWith('/devtools/') && !request.url?.startsWith('/json')) {
      response.writeHead(404).end(); return;
    }
    const proxy = http.request({ hostname: '127.0.0.1', port: upstream.port, path: request.url, method: request.method }, (result) => {
      response.writeHead(result.statusCode, result.headers);
      result.pipe(response);
    });
    proxy.on('error', () => { if (!response.headersSent) response.writeHead(502); response.end(); });
    request.pipe(proxy);
  });
  server.on('upgrade', (request, socket, head) => {
    let url;
    try { url = new URL(request.url, 'http://127.0.0.1'); } catch { socket.destroy(); return; }
    const pageTarget = url.pathname.startsWith('/devtools/page/') ? url.pathname.slice('/devtools/page/'.length) : null;
    const isRoot = url.pathname === browserPath;
    if (!allowedOrigin(request.headers.origin, port) || (!isRoot && !tabs.tabs.has(pageTarget))) {
      socket.destroy(); return;
    }
    sockets.handleUpgrade(request, socket, head, (client) => {
      if (isRoot) rootConnections += 1;
      connectCdp({ client, tabs, downloads, quit, pageTarget,
        upstreamUrl: isRoot ? upstreamUrl : `ws://127.0.0.1:${upstream.port}/devtools/page/${pageTarget}` });
      client.once('close', () => {
        if (isRoot && --rootConnections === 0) {
          setTimeout(() => { if (rootConnections === 0) quit(); }, 500).unref();
        }
      });
    });
  });
  await new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', resolve);
  });
  port = server.address().port;
  return { port, browserPath, close() {
    for (const client of sockets.clients) client.terminate();
    sockets.close();
    server.close();
  } };
}

module.exports = { startCdpServer, allowedOrigin };
