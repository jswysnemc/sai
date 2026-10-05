const http = require('node:http');

/**
 * 【桌面端】【服务定位】只接受 Sai 输出的本机随机端口及启动令牌
 * @param {string} line 后端的一行标准输出
 * @returns {URL|null} 合法服务地址，非启动行返回 null
 */
function parseStartupUrl(line) {
  const match = line.match(/^Sai Web: (http:\/\/127\.0\.0\.1:\d+\/\?token=[A-Za-z0-9_-]+)\s*$/);
  if (!match) return null;
  try {
    const url = new URL(match[1]);
    return Number(url.port) > 0 ? url : null;
  } catch {
    return null;
  }
}

/**
 * 【桌面端】【健康检查】直接访问回环服务，避免系统代理干扰
 * @param {string} origin 后端源地址
 * @param {number} timeout 单次请求超时毫秒数
 * @returns {Promise<boolean>} 是否返回 Sai 健康状态
 */
function checkHealth(origin, timeout = 1000) {
  return new Promise((resolve) => {
    const request = http.get(`${origin}/api/health`, (response) => {
      let body = '';
      response.setEncoding('utf8');
      response.on('data', (chunk) => {
        body += chunk;
        if (body.length > 4096) request.destroy();
      });
      response.on('error', () => resolve(false));
      response.on('end', () => {
        try {
          const result = JSON.parse(body);
          resolve(response.statusCode === 200 && result.ok === true && typeof result.version === 'string');
        } catch {
          resolve(false);
        }
      });
    });
    request.setTimeout(timeout, () => request.destroy());
    request.on('error', () => resolve(false));
  });
}

module.exports = { parseStartupUrl, checkHealth };
