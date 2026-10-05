/**
 * 【桌面构建】【二进制识别】从 ELF、PE 或 Mach-O 头解析操作系统和 CPU
 * @param {Buffer} bytes 可执行文件字节
 * @returns {string} 平台与架构组合，不支持时抛出错误
 */
function binaryTarget(bytes) {
  if (bytes.length < 64) throw new Error('后端可执行文件不完整');
  if (bytes.subarray(0, 4).equals(Buffer.from([0x7f, 0x45, 0x4c, 0x46]))) {
    if (bytes[4] !== 2 || bytes[5] !== 1) throw new Error('仅支持 64 位小端 ELF 后端');
    const machine = bytes.readUInt16LE(18);
    if (machine === 62) return 'linux-x64';
    if (machine === 183) return 'linux-arm64';
  } else if (bytes.toString('ascii', 0, 2) === 'MZ') {
    const offset = bytes.readUInt32LE(0x3c);
    if (offset + 6 > bytes.length || bytes.readUInt32LE(offset) !== 0x4550) throw new Error('Windows PE 文件头无效');
    if (bytes.readUInt16LE(offset + 4) === 0x8664) return 'win-x64';
  } else if (bytes.readUInt32LE(0) === 0xfeedfacf) {
    const cpu = bytes.readUInt32LE(4);
    if (cpu === 0x01000007) return 'mac-x64';
    if (cpu === 0x0100000c) return 'mac-arm64';
  }
  throw new Error('无法识别后端平台或 CPU 架构');
}

/**
 * 【桌面构建】【架构校验】拒绝把其他平台或架构的后端装入目标包
 * @param {Buffer} bytes 可执行文件字节
 * @param {string} expected 预期的平台与架构组合
 * @returns {void} 不匹配时抛出错误
 */
function verifyBinaryTarget(bytes, expected) {
  const actual = binaryTarget(bytes);
  if (actual !== expected) throw new Error(`后端实际为 ${actual}，目标安装包要求 ${expected}`);
}

module.exports = { binaryTarget, verifyBinaryTarget };
