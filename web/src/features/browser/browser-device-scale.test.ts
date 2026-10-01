import { describe, expect, it, vi } from "vitest";
import { browserDeviceScale, observeBrowserDeviceScale } from "./browser-device-scale";

/**
 * 【像素比测试】【窗口替身】模拟连续跨屏事件并记录查询监听，避免依赖真实显示设备。
 * @param ratio 初始像素比
 * @returns 模拟窗口、媒体查询列表与事件监听
 */
function createWindow(ratio: number) {
  const queries: { media: string; listeners: Set<() => void> }[] = [];
  const resize = new Set<() => void>();
  const target = {
    devicePixelRatio: ratio,
    addEventListener: (_type: string, listener: () => void) => resize.add(listener),
    removeEventListener: (_type: string, listener: () => void) => resize.delete(listener),
    matchMedia: (media: string) => {
      const listeners = new Set<() => void>();
      queries.push({ media, listeners });
      return {
        addEventListener: (_type: string, listener: () => void) => listeners.add(listener),
        removeEventListener: (_type: string, listener: () => void) => listeners.delete(listener)
      };
    }
  };
  return {
    target: target as unknown as Window,
    queries,
    resize,
    /** 设置替身的屏幕密度；value 为新像素比，返回无。 */
    setRatio: (value: number) => { target.devicePixelRatio = value; }
  };
}

describe("浏览器画面像素密度", () => {
  it("限制异常输入，保留常见屏幕缩放比例", () => {
    for (const ratio of [NaN, Infinity, -1, 0, 0.5]) expect(browserDeviceScale(ratio)).toBe(1);
    expect(browserDeviceScale(1.25000001)).toBe(1.25);
    expect(browserDeviceScale(1.5)).toBe(1.5);
    expect(browserDeviceScale(3)).toBe(2);
  });

  it("面板尺寸不变时，连续跨屏仍通知新像素比并释放旧查询", () => {
    const { target, queries, resize, setRatio } = createWindow(2);
    const changed = vi.fn();
    const stop = observeBrowserDeviceScale(changed, target);
    expect(changed).toHaveBeenLastCalledWith(2);
    for (const ratio of [1.5, 1, 2]) {
      const old = queries.at(-1)!;
      setRatio(ratio);
      [...old.listeners].forEach(listener => listener());
      expect(changed).toHaveBeenLastCalledWith(ratio);
      expect(old.listeners.size).toBe(0);
      expect(queries.at(-1)!.media).toBe(`(resolution: ${ratio}dppx)`);
    }
    stop();
    expect(resize.size).toBe(0);
    expect(queries.at(-1)!.listeners.size).toBe(0);
  });

  it("浏览器缩放触发同步，相同有效像素比不重复通知，但继续监听原始密度", () => {
    const { target, queries, resize, setRatio } = createWindow(3);
    const changed = vi.fn();
    const stop = observeBrowserDeviceScale(changed, target);
    setRatio(4);
    [...resize].forEach(listener => listener());
    expect(changed).toHaveBeenCalledTimes(1);
    expect(queries.at(-1)!.media).toBe("(resolution: 4dppx)");
    setRatio(1.25);
    [...resize].forEach(listener => listener());
    expect(changed).toHaveBeenLastCalledWith(1.25);
    stop();
  });
});
