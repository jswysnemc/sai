const path = require('node:path');

/**
 * 【桌面构建】【源码目录】定位本仓库根目录，允许 SAI_SOURCE_DIR 覆盖
 * @param {string} desktopRoot desktop 工程根目录
 * @returns {string} Sai 源码根目录
 */
function sourceDir(desktopRoot) {
  return path.resolve(process.env.SAI_SOURCE_DIR || path.join(desktopRoot, '..'));
}

/**
 * 【桌面构建】【暂存目录】按平台隔离后端与启动器，不进入安装包输出目录
 * @param {string} desktopRoot desktop 工程根目录
 * @param {string} targetKey 例如 linux-x64
 * @returns {string} 当前目标的暂存根目录
 */
function stagingDir(desktopRoot, targetKey) {
  return path.join(desktopRoot, '.staging', targetKey);
}

/**
 * 【桌面构建】【后端暂存】安装包收集用的 Sai 可执行文件目录
 * @param {string} desktopRoot desktop 工程根目录
 * @param {string} targetKey 例如 linux-x64
 * @returns {string} 后端暂存目录
 */
function backendStagingDir(desktopRoot, targetKey) {
  return path.join(stagingDir(desktopRoot, targetKey), 'backend');
}

/**
 * 【桌面构建】【启动器暂存】安装包收集用的浏览器启动器目录
 * @param {string} desktopRoot desktop 工程根目录
 * @param {string} targetKey 例如 linux-x64
 * @returns {string} 启动器暂存目录
 */
function hostStagingDir(desktopRoot, targetKey) {
  return path.join(stagingDir(desktopRoot, targetKey), 'desktop-host');
}

module.exports = { sourceDir, stagingDir, backendStagingDir, hostStagingDir };
