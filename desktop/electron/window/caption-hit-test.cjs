/** Windows 在主题色到达前使用的标题栏底色，与窗口 backgroundColor 一致。 */
const WINDOWS_FALLBACK = '#f7f7f7';

/**
 * 【桌面端】【标题栏命中】决定窗口按钮视图的背景色。
 *
 * Windows 无边框窗口对 alpha 为 0 的视图做穿透命中测试（HTTRANSPARENT）。
 * 点击会落到工作台顶行的拖动区，被当成 HTCAPTION。单击因此没有效果，
 * 也不会产生最小化、最大化、关闭对应的 WM_SYSCOMMAND。
 * 按钮视图用不透明底色才能收到点击。macOS 使用系统红绿灯，没有这块视图。
 *
 * @param {string} platform 进程平台
 * @param {string} [paper] 工作台纸色，仅接受 #rrggbb
 * @returns {string|null} 视图背景色；macOS 为 null
 */
function controlsBackground(platform, paper) {
  if (platform === 'darwin') return null;
  if (platform !== 'win32') return '#00000000';
  return /^#[\da-f]{6}$/i.test(paper || '') ? paper : WINDOWS_FALLBACK;
}

module.exports = { controlsBackground, WINDOWS_FALLBACK };
