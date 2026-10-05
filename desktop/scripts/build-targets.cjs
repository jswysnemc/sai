const TARGETS = {
  'linux-x64': 'x86_64-unknown-linux-gnu',
  'linux-arm64': 'aarch64-unknown-linux-gnu',
  'win-x64': 'x86_64-pc-windows-msvc',
  'mac-x64': 'x86_64-apple-darwin',
  'mac-arm64': 'aarch64-apple-darwin',
};

/**
 * 【桌面构建】【目标解析】统一 Electron 平台名称并校验支持的架构
 * @param {object} options 可选 platform 和 arch，默认使用当前主机
 * @returns {object} 平台、架构、Rust 三元组及文件名
 */
function resolveTarget({ platform = process.platform, arch = process.arch } = {}) {
  platform = ({ win32: 'win', darwin: 'mac' })[platform] || platform;
  const triple = TARGETS[`${platform}-${arch}`];
  if (!triple) throw new Error(`不支持的构建目标：${platform}-${arch}`);
  return { platform, arch, triple, key: `${platform}-${arch}`, binary: platform === 'win' ? 'sai.exe' : 'sai' };
}

/**
 * 【桌面构建】【主机校验】源码构建必须在对应主机进行，跨平台仅允许复用已验证产物
 * @param {object} target 请求的构建目标
 * @param {object} host 当前构建主机信息
 * @returns {void} 不匹配时抛出可操作的错误
 */
function requireNativeHost(target, host = resolveTarget()) {
  if (target.key !== host.key) {
    throw new Error(`源码构建 ${target.key} 需要对应平台及架构的主机；请使用多平台工作流，或用 --reuse 导入已编译的后端`);
  }
}

module.exports = { TARGETS, resolveTarget, requireNativeHost };
