import { useEffect, useState } from "react";
import { isDarkTheme, type ThemeId } from "./theme";

/** 当前生效的明暗外观 */
export type ThemeAppearance = "light" | "dark";

/**
 * 读取此刻生效的明暗外观。
 *
 * 主题标识写在 `documentElement` 的 data-theme 上，`system` 需要再查系统偏好。
 *
 * @returns 明暗外观标识
 */
function currentAppearance(): ThemeAppearance {
  const theme = (document.documentElement.dataset.theme ?? "system") as ThemeId;
  return isDarkTheme(theme) ? "dark" : "light";
}

/**
 * 订阅界面明暗外观。
 *
 * 供需要按明暗切换非 CSS 资源的组件使用，例如 mermaid 的渲染主题、
 * 第三方画布的配色——这些无法通过 CSS 变量表达，必须在渲染时拿到明暗结论。
 * 主题由设置页写入 data-theme，`system` 时还要跟随系统偏好，因此同时监听两处。
 *
 * @returns 当前明暗外观，主题或系统偏好变化时自动更新
 */
export function useThemeAppearance(): ThemeAppearance {
  const [appearance, setAppearance] = useState<ThemeAppearance>(currentAppearance);

  useEffect(() => {
    const sync = () => setAppearance(currentAppearance());
    // 1. 挂载后立刻校准一次，避免首帧与实际主题错位
    sync();
    // 2. 监听 data-theme 变化，覆盖用户在设置页切换主题
    const observer = new MutationObserver(sync);
    observer.observe(document.documentElement, { attributeFilter: ["data-theme"] });
    // 3. 监听系统深浅色变化，覆盖 system 主题下的操作系统切换
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    media.addEventListener("change", sync);
    return () => {
      observer.disconnect();
      media.removeEventListener("change", sync);
    };
  }, []);

  return appearance;
}
