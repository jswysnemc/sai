const { execFile } = require('node:child_process');

/** 字族列表缓存；读取失败不缓存，方便用户再次点击 */
let cachedFamilies = null;

/**
 * 【桌面端】【字体解析】拆开 fontconfig 的字族行。
 *
 * 一行里逗号分隔同一字体的不同语言名称，选择器需要把这些名称都列出来。
 * @param {string} text fc-list 的标准输出
 * @returns {string[]} 去重后按中文排序的字族
 */
function parseFontconfigFamilies(text) {
  const names = new Set();
  for (const line of String(text).split(/\r?\n/)) {
    for (const part of line.split(',')) {
      const family = part.trim();
      if (family) names.add(family);
    }
  }
  return sortFamilies(names);
}

/**
 * 【桌面端】【字体解析】从 Windows 字体注册表输出里取出字族名。
 * @param {string} text reg query 的标准输出
 * @returns {string[]} 去重后按中文排序的字族
 */
function parseWindowsFontNames(text) {
  const names = new Set();
  for (const line of String(text).split(/\r?\n/)) {
    const match = line.match(/^\s+(.+?)\s+REG_\w+\s+/);
    if (!match) continue;
    const family = match[1].replace(/\s+\((?:TrueType|OpenType|All Res)\)$/i, '').trim();
    if (family) names.add(family);
  }
  return sortFamilies(names);
}

/**
 * 【桌面端】【字体排序】按中文区域设置比较字族名。
 * @param {Iterable<string>} names 字族名
 * @returns {string[]} 排序结果
 */
function sortFamilies(names) {
  return [...names].sort((left, right) => left.localeCompare(right, 'zh'));
}

/**
 * 【桌面端】【字体读取】按当前系统列出已安装字族。
 * @returns {Promise<string[]>} 字族名；命令失败时为空数组
 */
async function listInstalledFontFamilies() {
  if (cachedFamilies) return cachedFamilies;
  const families = await readPlatformFonts();
  if (families.length > 0) cachedFamilies = families;
  return families;
}

/**
 * 【桌面端】【字体读取】调用系统字库命令。
 * @returns {Promise<string[]>} 字族名
 */
function readPlatformFonts() {
  if (process.platform === 'win32') return readWindowsFonts();
  if (process.platform === 'darwin') return readDarwinFonts();
  return readFontconfigFonts();
}

/**
 * 【桌面端】【字体读取】用 fontconfig 列出字族。
 * @returns {Promise<string[]>} 字族名
 */
function readFontconfigFonts() {
  return execText('fc-list', [':', 'family']).then(parseFontconfigFamilies);
}

/**
 * 【桌面端】【字体读取】macOS 优先使用 fontconfig，没有时退回系统字体描述。
 * @returns {Promise<string[]>} 字族名
 */
async function readDarwinFonts() {
  try {
    return await readFontconfigFonts();
  } catch {
    const text = await execText('system_profiler', ['-json', 'SPFontsDataType']);
    const names = new Set();
    const payload = JSON.parse(text);
    const fonts = Array.isArray(payload?.SPFontsDataType) ? payload.SPFontsDataType : [];
    for (const font of fonts) {
      const family = typeof font?.typefaces?.[0]?._name === 'string' ? font.typefaces[0]._name : font?._name;
      if (typeof family === 'string' && family.trim()) names.add(family.trim());
    }
    return sortFamilies(names);
  }
}

/**
 * 【桌面端】【字体读取】读取当前用户和本机的 Windows 字体注册表。
 * @returns {Promise<string[]>} 字族名
 */
async function readWindowsFonts() {
  const keys = [
    'HKCU\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Fonts',
    'HKLM\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Fonts',
  ];
  const chunks = await Promise.all(keys.map((key) => execText('reg', ['query', key]).catch(() => '')));
  return parseWindowsFontNames(chunks.join('\n'));
}

/**
 * 【桌面端】【命令执行】运行字库查询并收集标准输出。
 * @param {string} file 可执行文件
 * @param {string[]} args 参数
 * @returns {Promise<string>} 标准输出
 */
function execText(file, args) {
  return new Promise((resolve, reject) => {
    execFile(file, args, { windowsHide: true, timeout: 15000, maxBuffer: 8 * 1024 * 1024 }, (error, stdout) => {
      if (error) reject(error);
      else resolve(String(stdout));
    });
  });
}

module.exports = {
  parseFontconfigFamilies,
  parseWindowsFontNames,
  listInstalledFontFamilies,
};
