import { isApplePlatform } from "../../../shared/mod-key";

/** 页面加载时的随机起点，保证每次打开看到不同技巧，但不随时间闪烁。 */
const PROCESS_SEED =
  (typeof performance !== "undefined" ? Math.floor(performance.now()) : Date.now()) ^
  (Date.now() & 0xffff);

/**
 * 返回当前语言下的全部输入框小技巧。
 *
 * 粘贴快捷键在调用时读取平台，避免模块加载阶段把 Ctrl/⌘ 写死。
 *
 * @param locale 当前界面语言
 * @returns 提示文案列表
 */
export function composerTips(locale: string): string[] {
  const paste = isApplePlatform() ? "⌘+V" : "Ctrl+V";
  const pairs: Array<[string, string]> = [
    ["Enter sends · Shift+Enter inserts a new line", "Enter 发送 · Shift+Enter 换行"],
    ["Use @ to mention workspace files", "用 @ 提及工作区文件"],
    ["Use /skill-name to attach a skill", "用 /技能名 附加技能"],
    [`Paste images into the composer with ${paste}`, `用 ${paste} 把图片粘贴进输入框`],
    ["Click the paperclip to attach images", "点回形针图标可附加图片"],
    ["Pick model and thinking level next to the composer", "在输入框旁选择模型与思考等级"],
    ["Modes: yolo · audit · auto · plan", "模式：yolo · audit · auto · plan"],
    ["Open Settings → Runtime for notifications and default modes", "设置 → 运行参数 可配置通知与默认模式"],
    ["Click images to open the lightbox preview", "点击图片可打开灯箱预览"],
    ["Use /goal to create or update a persistent goal", "用 /goal 创建或更新持久目标"],
    ["Use /rename to name the current session", "用 /rename 为当前会话命名"]
  ];
  const zh = locale.startsWith("zh");
  return pairs.map(([en, zhText]) => (zh ? zhText : en));
}

/**
 * 返回本次页面应展示的一条输入框操作小技巧。
 *
 * @param locale 当前界面语言
 * @returns 提示文案
 */
export function currentComposerTip(locale: string): string {
  const tips = composerTips(locale);
  if (tips.length === 0) return "";
  return tips[(PROCESS_SEED >>> 0) % tips.length];
}
