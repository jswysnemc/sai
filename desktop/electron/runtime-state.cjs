const fs = require('node:fs');

/**
 * 【桌面端】【端口恢复】恢复上次使用的端口，维持前端偏好设置的源地址
 * @param {string} file 运行状态文件路径
 * @returns {number} 上次端口，缺失或损坏时返回 0
 */
function readPort(file) {
  try {
    const { port } = JSON.parse(fs.readFileSync(file, 'utf8'));
    return Number.isInteger(port) && port > 1023 && port <= 65535 ? port : 0;
  } catch {
    return 0;
  }
}

/**
 * 【桌面端】【端口保存】原子保存本次端口，不记录认证信息
 * @param {string} file 运行状态文件路径
 * @param {number} port 本次服务端口
 * @returns {void} 无返回值
 */
function savePort(file, port) {
  fs.writeFileSync(`${file}.tmp`, `${JSON.stringify({ port })}\n`, { mode: 0o600 });
  fs.renameSync(`${file}.tmp`, file);
}

module.exports = { readPort, savePort };
