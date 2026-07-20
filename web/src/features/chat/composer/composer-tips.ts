/** 进程 / 页面加载时的轮询起点，保证每次启动偏移不同。 */
const PROCESS_SEED =
  (typeof performance !== "undefined" ? Math.floor(performance.now()) : Date.now()) ^
  (Date.now() & 0xffff);

const TIP_PAIRS: Array<[string, string]> = [
  ["Tab is for modes in TUI · Enter sends · Shift+Enter for a new line", "TUI 中 Tab 切换模式 · Enter 发送 · Shift+Enter 换行"],
  ["Type / for commands · /model · /auto · /help", "输入 / 打开命令 · /model · /auto · /help"],
  ["Prefix ! in TUI to run a local shell command", "TUI 中以 ! 开头可执行本地 shell 命令"],
  ["Ctrl+O expands or collapses command / thinking output in TUI", "TUI 中 Ctrl+O 展开或折叠命令输出 / 思考段落"],
  ["Paste images into the composer with Ctrl+V", "用 Ctrl+V 把图片粘贴进输入框"],
  ["Use @ to mention workspace files", "用 @ 提及工作区文件"],
  ["Use /skill-name to attach a skill", "用 /技能名 附加技能"],
  ["Modes: yolo · audit · auto · plan", "模式：yolo · audit · auto · plan"],
  ["Open Settings → Runtime for notifications and default modes", "设置 → 运行参数 可配置通知与默认模式"],
  ["Click images to open the lightbox preview", "点击图片可打开灯箱预览"]
];

/**
 * 返回当前应展示的输入框操作小技巧。
 *
 * @param locale 当前界面语言
 * @param nowMs 当前时间戳，便于测试注入
 * @returns 提示文案
 */
export function currentComposerTip(locale: string, nowMs = Date.now()): string {
  const zh = locale.startsWith("zh");
  const tips = TIP_PAIRS.map(([en, zhText]) => (zh ? zhText : en));
  if (tips.length === 0) return "";
  const slot = Math.floor(nowMs / 8_000);
  const index = (PROCESS_SEED + slot) >>> 0;
  return tips[index % tips.length];
}

/**
 * 返回提示轮询间隔（毫秒）。
 */
export function composerTipIntervalMs(): number {
  return 8_000;
}
