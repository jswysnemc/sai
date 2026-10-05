const { test } = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { resolveTarget, requireNativeHost } = require('../scripts/build-targets.cjs');
const { binaryTarget, verifyBinaryTarget } = require('../scripts/binary-target.cjs');
const { sourceDir, backendStagingDir, hostStagingDir } = require('../scripts/repo-paths.cjs');

/**
 * 【桌面测试】【文件头样本】创建对应格式和架构的最小可执行头
 * @param {string} target 测试目标
 * @returns {Buffer} ELF、PE 或 Mach-O 头
 */
function header(target) {
  const bytes = Buffer.alloc(128);
  if (target.startsWith('linux')) {
    Buffer.from([0x7f, 0x45, 0x4c, 0x46, 2, 1]).copy(bytes);
    bytes.writeUInt16LE(target.endsWith('x64') ? 62 : 183, 18);
  } else if (target === 'win-x64') {
    bytes.write('MZ');
    bytes.writeUInt32LE(64, 0x3c);
    bytes.writeUInt32LE(0x4550, 64);
    bytes.writeUInt16LE(0x8664, 68);
  } else {
    bytes.writeUInt32LE(0xfeedfacf, 0);
    bytes.writeUInt32LE(target.endsWith('x64') ? 0x01000007 : 0x0100000c, 4);
  }
  return bytes;
}

test('目标映射统一系统名称，并拒绝未支持的组合', () => {
  assert.equal(resolveTarget({ platform: 'win32', arch: 'x64' }).triple, 'x86_64-pc-windows-msvc');
  assert.equal(resolveTarget({ platform: 'darwin', arch: 'arm64' }).key, 'mac-arm64');
  assert.throws(() => resolveTarget({ platform: 'linux', arch: 'ia32' }), /不支持/);
  assert.throws(() => requireNativeHost(resolveTarget({ platform: 'mac', arch: 'arm64' }),
    resolveTarget({ platform: 'linux', arch: 'x64' })), /对应平台/);
});

test('按文件头识别五个构建目标，拒绝错误架构及损坏的 PE 偏移', () => {
  for (const target of ['linux-x64', 'linux-arm64', 'win-x64', 'mac-x64', 'mac-arm64']) {
    assert.equal(binaryTarget(header(target)), target);
    assert.doesNotThrow(() => verifyBinaryTarget(header(target), target));
  }
  assert.throws(() => verifyBinaryTarget(header('linux-x64'), 'win-x64'), /实际为/);
  assert.throws(() => verifyBinaryTarget(header('mac-x64'), 'mac-arm64'), /实际为/);
  const corrupt = header('win-x64');
  corrupt.writeUInt32LE(65535, 0x3c);
  assert.throws(() => binaryTarget(corrupt), /文件头无效/);
  assert.throws(() => binaryTarget(Buffer.alloc(4)), /不完整/);
});

test('源码目录默认指向本仓库根，暂存目录与安装包输出分离', () => {
  const previous = process.env.SAI_SOURCE_DIR;
  delete process.env.SAI_SOURCE_DIR;
  try {
    const desktopRoot = path.resolve(__dirname, '..');
    assert.equal(sourceDir(desktopRoot), path.resolve(desktopRoot, '..'));
    assert.equal(backendStagingDir(desktopRoot, 'linux-x64'), path.join(desktopRoot, '.staging', 'linux-x64', 'backend'));
    assert.equal(hostStagingDir(desktopRoot, 'win-x64'), path.join(desktopRoot, '.staging', 'win-x64', 'desktop-host'));
  } finally {
    if (previous === undefined) delete process.env.SAI_SOURCE_DIR;
    else process.env.SAI_SOURCE_DIR = previous;
  }
});
