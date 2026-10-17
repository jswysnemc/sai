const { test } = require('node:test');
const assert = require('node:assert/strict');
const { parseFontconfigFamilies, parseWindowsFontNames, listInstalledFontFamilies } = require('../electron/fonts.cjs');

test('fontconfig 输出按逗号拆开多语言字族名', () => {
  const families = parseFontconfigFamilies([
    'Sarasa Mono SC,等距更纱黑体 SC',
    'Noto Sans CJK SC',
    'Sarasa Mono SC',
    '',
  ].join('\n'));
  assert.deepEqual(families, ['等距更纱黑体 SC', 'Noto Sans CJK SC', 'Sarasa Mono SC']);
});

test('Windows 字体注册表去掉字型后缀', () => {
  const families = parseWindowsFontNames([
    'HKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Fonts',
    '    Arial (TrueType)    REG_SZ    arial.ttf',
    '    Segoe UI Bold (TrueType)    REG_SZ    segoeuib.ttf',
    '    微软雅黑 (TrueType)    REG_SZ    msyh.ttc',
  ].join('\n'));
  assert.deepEqual(families, ['微软雅黑', 'Arial', 'Segoe UI Bold']);
});

test('当前系统能列出已安装字族', async () => {
  const families = await listInstalledFontFamilies();
  assert.ok(families.length > 0);
  assert.equal(new Set(families).size, families.length);
});
