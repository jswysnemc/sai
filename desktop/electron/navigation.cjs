/**
 * 【桌面端】【导航校验】只允许当前后端源地址留在桌面窗口
 * @param {string} value 待访问地址
 * @param {string} origin 当前后端源地址
 * @returns {boolean} 是否为不含用户信息的同源 HTTP 地址
 */
function isInternal(value, origin) {
  try {
    const url = new URL(value);
    return url.origin === origin && url.protocol === 'http:' && !url.username && !url.password;
  } catch {
    return false;
  }
}

/**
 * 【桌面端】【外部链接】限制系统浏览器可接收的协议
 * @param {string} value 待打开地址
 * @returns {boolean} 是否允许交给系统浏览器或邮件客户端
 */
function isExternal(value) {
  try {
    const url = new URL(value);
    return ['https:', 'http:', 'mailto:'].includes(url.protocol) && !url.username && !url.password;
  } catch {
    return false;
  }
}

module.exports = { isInternal, isExternal };
