/** 页面加载时的轮询起点，保证每次打开偏移不同。 */
const PROCESS_SEED =
  (typeof performance !== "undefined" ? Math.floor(performance.now()) : Date.now()) ^
  (Date.now() & 0xffff);

/** Web 输入框专属小技巧（不含 TUI 快捷键）。 */
const TIP_PAIRS: Array<[string, string]> = [
  ["Enter sends · Shift+Enter inserts a new line", "Enter 发送 · Shift+Enter 换行"],
  ["Use @ to mention workspace files", "用 @ 提及工作区文件"],
  ["Use /skill-name to attach a skill", "用 /技能名 附加技能"],
  ["Paste images into the composer with Ctrl+V", "用 Ctrl+V 把图片粘贴进输入框"],
  ["Click the paperclip to attach local files", "点回形针图标可附加本地文件"],
  ["Pick model and thinking level next to the composer", "在输入框旁选择模型与思考等级"],
  ["Modes: yolo · audit · auto · plan", "模式：yolo · audit · auto · plan"],
  ["Open Settings → Runtime for notifications and default modes", "设置 → 运行参数 可配置通知与默认模式"],
  ["Click images to open the lightbox preview", "点击图片可打开灯箱预览"],
  ["Use /goal to create or update a persistent goal", "用 /goal 创建或更新持久目标"]
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
