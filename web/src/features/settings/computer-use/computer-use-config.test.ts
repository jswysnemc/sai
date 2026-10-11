import { describe, expect, it } from "vitest";
import type { AppConfig } from "../../../api/contracts";
import {
  clampTimeout,
  DEFAULT_COMPUTER_USE,
  readComputerUse,
  shouldReplaceDriverPath,
  writeComputerUse
} from "./computer-use-config";

describe("computer-use config helpers", () => {
  it("falls back to disabled defaults", () => {
    expect(readComputerUse({} as unknown as AppConfig)).toEqual(DEFAULT_COMPUTER_USE);
  });

  it("clamps timeout to the backend range", () => {
    expect(clampTimeout(0)).toBe(1_000);
    expect(clampTimeout(200_000)).toBe(120_000);
    expect(clampTimeout(Number.NaN)).toBe(30_000);
  });

  it("writes a trimmed driver path", () => {
    const next = writeComputerUse({} as unknown as AppConfig, { enabled: true, driver_path: "  /opt/cua-driver  ", timeout_ms: 5_000 });
    expect(next.computer_use).toEqual({
      enabled: true,
      driver_path: "/opt/cua-driver",
      timeout_ms: 5_000
    });
  });

  it("replaces only default or previously installed paths", () => {
    expect(shouldReplaceDriverPath("cua-driver", "/data/computer-use/cua-driver")).toBe(true);
    expect(shouldReplaceDriverPath("/data/computer-use/cua-driver", "/data/computer-use/cua-driver")).toBe(true);
    expect(shouldReplaceDriverPath("/opt/custom", "/data/computer-use/cua-driver")).toBe(false);
  });
});
