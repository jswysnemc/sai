import { useEffect } from "react";
import { loadRunningBackgroundWork } from "./session-close-guard";

/**
 * 当前网页会话有后台工作时，关闭标签页会弹出离开确认。
 *
 * 只看这个页面正在显示的会话，不包含终端里其他并行会话。
 * 浏览器不允许自定义这段文案。没有后台工作时不拦截。
 *
 * @param sessionId 当前网页会话；还没有打开会话时不拦截
 * @returns 无
 */
export function useBackgroundWorkExitWarning(sessionId?: string): void {
  useEffect(() => {
    let armed = false;
    /**
     * 刷新当前会话是否还有运行中的后台工作。
     *
     * @returns 无返回值
     */
    const refresh = () => {
      if (!sessionId) {
        armed = false;
        return;
      }
      void loadRunningBackgroundWork(sessionId).then((items) => {
        armed = items.length > 0;
      });
    };
    /**
     * 有后台工作时阻止直接关闭标签页。
     *
     * @param event 页面离开事件
     * @returns 无返回值
     */
    const onBeforeUnload = (event: BeforeUnloadEvent) => {
      if (!armed) return;
      event.preventDefault();
      event.returnValue = "";
    };
    refresh();
    const timer = window.setInterval(refresh, 5_000);
    window.addEventListener("beforeunload", onBeforeUnload);
    return () => {
      window.clearInterval(timer);
      window.removeEventListener("beforeunload", onBeforeUnload);
    };
  }, [sessionId]);
}
