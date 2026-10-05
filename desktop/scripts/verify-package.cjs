const fs = require('node:fs/promises');
const path = require('node:path');
const { createHash } = require('node:crypto');
const { verifyBinaryTarget } = require('./binary-target.cjs');
const { backendStagingDir, hostStagingDir } = require('./repo-paths.cjs');

/**
 * 【桌面构建】【打包检查】拒绝缺失、架构不符或被替换的后端
 * @param {object} context electron-builder 提供的目标平台上下文
 * @returns {Promise<void>} 验证失败时终止打包
 */
module.exports = async function verifyPackage(context) {
  const arch = ['ia32', 'x64', 'armv7l', 'arm64', 'universal'][context.arch];
  const platform = context.electronPlatformName === 'win32' ? 'win'
    : context.electronPlatformName === 'darwin' ? 'mac' : 'linux';
  const key = `${platform}-${arch}`;
  const directory = backendStagingDir(context.packager.projectDir, key);
  const manifest = JSON.parse(await fs.readFile(path.join(directory, 'manifest.json'), 'utf8').catch(() => {
    throw new Error(`缺少 ${key} 后端，请先在对应平台运行 pnpm backend:build`);
  }));
  if (manifest.platform !== platform || manifest.arch !== arch) throw new Error('后端平台或架构与安装包不符');
  const binary = await fs.readFile(path.join(directory, platform === 'win' ? 'sai.exe' : 'sai'));
  verifyBinaryTarget(binary, key);
  if (createHash('sha256').update(binary).digest('hex') !== manifest.sha256) {
    throw new Error('Sai 后端校验失败，请重新构建');
  }
  const hostDirectory = hostStagingDir(context.packager.projectDir, key);
  const hostManifest = JSON.parse(await fs.readFile(path.join(hostDirectory, 'manifest.json'), 'utf8'));
  if (hostManifest.platform !== platform || hostManifest.arch !== arch) throw new Error('浏览器启动器平台或架构与安装包不符');
  const launcher = await fs.readFile(path.join(hostDirectory, platform === 'win' ? 'sai-browser-launcher.exe' : 'sai-browser-launcher'));
  verifyBinaryTarget(launcher, key);
  if (createHash('sha256').update(launcher).digest('hex') !== hostManifest.sha256) throw new Error('桌面浏览器启动器校验失败');
};
