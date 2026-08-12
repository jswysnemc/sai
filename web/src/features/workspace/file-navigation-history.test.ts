import { describe, expect, it } from "vitest";
import {
  canGoBack,
  canGoForward,
  EMPTY_FILE_NAVIGATION_HISTORY,
  goBack,
  goForward,
  recordFileVisit
} from "./file-navigation-history";

describe("file navigation history", () => {
  it("按访问顺序入栈且不重复记录当前文件", () => {
    let history = EMPTY_FILE_NAVIGATION_HISTORY;
    history = recordFileVisit(history, "a.ts");
    history = recordFileVisit(history, "b.ts");
    history = recordFileVisit(history, "b.ts");

    expect(history.stack).toEqual(["a.ts", "b.ts"]);
    expect(history.index).toBe(1);
    expect(canGoBack(history)).toBe(true);
    expect(canGoForward(history)).toBe(false);
  });

  it("后退与前进在栈内移动", () => {
    let history = recordFileVisit(recordFileVisit(recordFileVisit(EMPTY_FILE_NAVIGATION_HISTORY, "a.ts"), "b.ts"), "c.ts");

    const back = goBack(history);
    expect(back?.path).toBe("b.ts");
    history = back!.history;
    expect(canGoForward(history)).toBe(true);

    const forward = goForward(history);
    expect(forward?.path).toBe("c.ts");
    expect(canGoForward(forward!.history)).toBe(false);
  });

  it("中段位置的新访问截断前进分支", () => {
    let history = recordFileVisit(recordFileVisit(EMPTY_FILE_NAVIGATION_HISTORY, "a.ts"), "b.ts");
    history = goBack(history)!.history;
    history = recordFileVisit(history, "c.ts");

    expect(history.stack).toEqual(["a.ts", "c.ts"]);
    expect(canGoForward(history)).toBe(false);
  });

  it("空历史不可后退或前进", () => {
    expect(canGoBack(EMPTY_FILE_NAVIGATION_HISTORY)).toBe(false);
    expect(canGoForward(EMPTY_FILE_NAVIGATION_HISTORY)).toBe(false);
    expect(goBack(EMPTY_FILE_NAVIGATION_HISTORY)).toBeNull();
    expect(goForward(EMPTY_FILE_NAVIGATION_HISTORY)).toBeNull();
  });
});
