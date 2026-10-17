import { useCallback, useState } from "react";
import { canQueryLocalFonts, readLocalFontFamilies } from "./local-fonts";

export type LocalFontStatus = "idle" | "loading" | "ready" | "denied" | "unavailable";

/**
 * 读取并缓存本机字族，供字体设置里的三个选择器共用。
 *
 * @returns 字族列表、读取状态，以及在用户点击后重新读取的方法
 */
export function useLocalFontFamilies() {
  const [families, setFamilies] = useState<string[]>([]);
  const [status, setStatus] = useState<LocalFontStatus>(canQueryLocalFonts() ? "idle" : "unavailable");

  const reload = useCallback(async () => {
    if (!canQueryLocalFonts()) {
      setStatus("unavailable");
      return;
    }
    setStatus("loading");
    try {
      setFamilies(await readLocalFontFamilies());
      setStatus("ready");
    } catch {
      setStatus("denied");
    }
  }, []);

  return { families, status, reload };
}
