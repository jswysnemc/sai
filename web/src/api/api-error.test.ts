import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiError, LocalizedError, localizeApiErrorMessage, localizeApiMessage, toDisplayError } from "./api-error";
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

  it("本地化 Git 操作成功消息", () => {
    expect(localizeApiMessage("repository initialized", "zh-CN")).toBe("仓库已初始化");
    expect(localizeApiMessage("repository initialized", "en-US")).toBe("repository initialized");
  });

  it("本地化 Git 工作区固定错误", () => {
    expect(localizeApiMessage("current directory is not a Git repository", "zh-CN"))
      .toBe("当前目录不是 Git 仓库");
    expect(localizeApiMessage("failed to read .gitignore: permission denied", "zh-CN"))
      .toBe("读取 .gitignore 失败：permission denied");
  });

  it("本地化其他 Web API 固定消息", () => {
    expect(localizeApiMessage("Gateway sessions", "zh-CN")).toBe("网关会话");
    expect(localizeApiMessage("model endpoint returned no result", "zh-CN"))
      .toBe("模型接口未返回结果");
    expect(localizeApiMessage("verification code rejected; enter it again", "zh-CN"))
      .toBe("验证码被拒绝，请重新输入");
    expect(localizeApiMessage("unknown login status: blocked", "zh-CN"))
      .toBe("未知登录状态：blocked");
    expect(localizeApiMessage(
      "conversation has been compacted multiple times; start a focused session if details become distorted",
      "zh-CN"
    )).toBe("当前会话已经多次压缩；如果细节开始失真，请新建聚焦会话继续");
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
