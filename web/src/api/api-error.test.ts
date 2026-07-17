import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiError, localizeApiErrorMessage } from "./api-error";
import { LOCALE_STORAGE_KEY } from "../features/i18n/locale";

describe("ApiError", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("本地化服务端目录错误", () => {
    expect(localizeApiErrorMessage("directory does not exist: /srv/app", "zh-CN"))
      .toBe("目录不存在：/srv/app");
    expect(localizeApiErrorMessage("directory does not exist: /srv/app", "en-US"))
      .toBe("directory does not exist: /srv/app");
  });

  it("读取 message 时采用最新界面语言", () => {
    const values = new Map<string, string>();
    vi.stubGlobal("window", {
      localStorage: {
        getItem: (key: string) => values.get(key) ?? null,
        setItem: (key: string, value: string) => values.set(key, value)
      }
    });
    vi.stubGlobal("navigator", { languages: [] });
    const error = new ApiError("directory name is empty");
    window.localStorage.setItem(LOCALE_STORAGE_KEY, "en-US");
    expect(error.message).toBe("directory name is empty");
    window.localStorage.setItem(LOCALE_STORAGE_KEY, "zh-CN");
    expect(error.message).toBe("目录名称不能为空");
  });
});
