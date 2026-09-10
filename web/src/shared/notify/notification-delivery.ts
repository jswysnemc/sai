import type { NotificationMessage } from "./notification-client";

/**
 * 【通知投递】【浏览器接口】按宿主已校验的数据投递，不修改正文或重新计算业务开关。
 * @param notification 纯策略返回的通知
 * @param signal 会话切换、卸载或超时时停止本次投递
 * @returns 无；浏览器权限或音频错误不影响对话
 */
export function deliverNotification(notification: NotificationMessage, signal: AbortSignal): void {
  if (signal.aborted || typeof window === "undefined") return;
  // 1. 【通知投递】【权限等待】权限结果迟到时再次检查取消信号
  if (notification.desktop && "Notification" in window) {
    const show = () => {
      if (signal.aborted) return;
      try {
        new Notification(notification.title, { body: notification.body, silent: true });
      } catch {
        // 【通知投递】【受限环境】系统拒绝投递时保持当前对话
      }
    };
    if (Notification.permission === "granted") {
      show();
    } else if (Notification.permission === "default") {
      try {
        void Notification.requestPermission()
          .then((permission) => { if (permission === "granted") show(); })
          .catch(() => {});
      } catch {
        // 【通知投递】【权限失败】兼容同步拒绝权限请求的浏览器
      }
    }
  }
  // 2. 【通知投递】【声音独立】声音不依赖系统通知权限
  if (notification.sound) playChime(signal);
}

/**
 * 【通知投递】【提示音】使用 Web Audio 播放固定短音，并在结束或取消时关闭设备。
 * @param signal 本次投递的取消信号
 * @returns 无；自动播放限制由浏览器决定
 */
function playChime(signal: AbortSignal): void {
  let context: AudioContext | undefined;
  let oscillator: OscillatorNode | undefined;
  let closed = false;
  const close = () => {
    if (closed) return;
    closed = true;
    signal.removeEventListener("abort", stop);
    if (context) void context.close().catch(() => {});
  };
  const stop = () => {
    try {
      oscillator?.stop();
    } catch {
      // 【通知投递】【音频回收】设备可能已经停止
    }
    close();
  };
  try {
    const AudioCtx = window.AudioContext
      || (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
    if (!AudioCtx || signal.aborted) return;
    context = new AudioCtx();
    oscillator = context.createOscillator();
    const gain = context.createGain();
    oscillator.type = "sine";
    oscillator.frequency.value = 880;
    gain.gain.value = 0.0001;
    oscillator.connect(gain);
    gain.connect(context.destination);
    const now = context.currentTime;
    gain.gain.exponentialRampToValueAtTime(0.08, now + 0.02);
    gain.gain.exponentialRampToValueAtTime(0.0001, now + 0.28);
    oscillator.onended = close;
    signal.addEventListener("abort", stop, { once: true });
    oscillator.start(now);
    oscillator.stop(now + 0.3);
  } catch {
    close();
  }
}
