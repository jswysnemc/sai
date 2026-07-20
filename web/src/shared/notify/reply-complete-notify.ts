import type { NotificationConfig } from "../../api/contracts";

/**
 * 在浏览器中发送答复完成通知（可选声音）。
 *
 * @param settings 通知配置；缺省视为启用且有声
 * @param title 通知标题
 * @param body 通知正文
 */
export function notifyReplyComplete(
  settings: NotificationConfig | undefined | null,
  title: string,
  body: string
): void {
  const enabled = settings?.enabled ?? true;
  const sound = settings?.sound ?? true;
  if (!enabled && !sound) return;

  // 1. 桌面 / 浏览器通知（需权限）
  if (enabled && typeof window !== "undefined" && "Notification" in window) {
    const show = () => {
      try {
        new Notification(title, {
          body: body.slice(0, 240),
          silent: true
        });
      } catch {
        // 忽略受限环境
      }
    };
    if (Notification.permission === "granted") {
      show();
    } else if (Notification.permission === "default") {
      void Notification.requestPermission().then((permission) => {
        if (permission === "granted") show();
      });
    }
  }

  // 2. 提示音与通知气泡解耦，便于单独开关
  if (sound) {
    playReplyChime();
  }
}

/**
 * 用 Web Audio 播放短提示音。
 */
function playReplyChime(): void {
  try {
    const AudioCtx =
      window.AudioContext ||
      (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
    if (!AudioCtx) return;
    const ctx = new AudioCtx();
    const oscillator = ctx.createOscillator();
    const gain = ctx.createGain();
    oscillator.type = "sine";
    oscillator.frequency.value = 880;
    gain.gain.value = 0.0001;
    oscillator.connect(gain);
    gain.connect(ctx.destination);
    const now = ctx.currentTime;
    gain.gain.exponentialRampToValueAtTime(0.08, now + 0.02);
    gain.gain.exponentialRampToValueAtTime(0.0001, now + 0.28);
    oscillator.start(now);
    oscillator.stop(now + 0.3);
    oscillator.onended = () => {
      void ctx.close();
    };
  } catch {
    // 忽略自动播放策略限制
  }
}
