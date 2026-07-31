import { describe, expect, it } from "vitest";
import { formatContextCacheDetail } from "./system-usage";

describe("formatContextCacheDetail", () => {
  it("缓存写入量为零时仍展示完整读写明细", () => {
    const detail = formatContextCacheDetail(
      { hit_tokens: 800, miss_tokens: 200, write_tokens: 0 },
      (_en, zh) => zh
    );

    expect(detail).toBe("800 命中 · 200 未命中 · 0 写入");
  });
});
