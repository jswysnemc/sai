import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiError, LocalizedError, localizeApiErrorMessage, toDisplayError } from "./api-error";
import { LOCALE_STORAGE_KEY } from "../features/i18n/locale";

describe("ApiError", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("本地化服务端目录错误", () => {
    expect(localizeApiErrorMessage("directory does not exist: /srv/app", "zh-CN"))
      .toBe("目录不存在：/srv/app");
    expect(localizeApiErrorMessage("directory does not exist: /srv/app", "en-US"))
      .toBe("directory does not exist: /srv/app");
  });

  it("本地化未取消提问时缺少答案的错误", () => {
    expect(localizeApiErrorMessage("answers are required unless cancelled", "zh-CN"))
      .toBe("未取消提问时必须提供答案");
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

  it("转换界面错误时保留原始 ApiError 对象", () => {
    const values = new Map<string, string>();
    vi.stubGlobal("window", {
      localStorage: {
        getItem: (key: string) => values.get(key) ?? null,
        setItem: (key: string, value: string) => values.set(key, value)
      }
    });
    vi.stubGlobal("navigator", { languages: [] });
    const error = new ApiError("directory name is empty");
    const storedError = toDisplayError(error, "Operation failed", "操作失败");

    expect(storedError).toBe(error);
    window.localStorage.setItem(LOCALE_STORAGE_KEY, "en-US");
    expect(storedError.message).toBe("directory name is empty");
    window.localStorage.setItem(LOCALE_STORAGE_KEY, "zh-CN");
    expect(storedError.message).toBe("目录名称不能为空");
  });

  it("错误对象存入状态后仍按最新界面语言读取兜底文案", () => {
    const values = new Map<string, string>();
    vi.stubGlobal("window", {
      localStorage: {
        getItem: (key: string) => values.get(key) ?? null,
        setItem: (key: string, value: string) => values.set(key, value)
      }
    });
    vi.stubGlobal("navigator", { languages: [] });
    const storedError: Error = toDisplayError(null, "Operation failed", "操作失败");

    expect(storedError).toBeInstanceOf(LocalizedError);
    window.localStorage.setItem(LOCALE_STORAGE_KEY, "en-US");
    expect(storedError.message).toBe("Operation failed");
    window.localStorage.setItem(LOCALE_STORAGE_KEY, "zh-CN");
    expect(storedError.message).toBe("操作失败");
  });
});
