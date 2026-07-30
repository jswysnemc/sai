import { describe, expect, it } from "vitest";
import type { EngineStatusResponse } from "../../../api/contracts";
import {
  engineConnectionLabel,
  hasHandshake,
  resolveEngineConnectionState
} from "./engine-connection-state";

/**
 * 构造带或不带握手结果的内核状态。
 *
 * @param handshaked 是否已完成握手
 * @returns 内核运行状态
 */
function status(handshaked: boolean): EngineStatusResponse {
  return {
    engine: "claude_code",
    label: "Claude Code",
    external: true,
    unavailable_features: [],
    acp_runtime: handshaked ? ({ config_options: [] } as never) : undefined
  } as EngineStatusResponse;
}

describe("engine connection state", () => {
  it("prefers the in-flight request over cached results", () => {
    expect(
      resolveEngineConnectionState({ status: status(true), connecting: true, failed: false })
    ).toBe("connecting");
  });

  it("reports connected once a handshake exists", () => {
    expect(
      resolveEngineConnectionState({ status: status(true), connecting: false, failed: false })
    ).toBe("connected");
  });

  it("clears the failure once a handshake succeeds", () => {
    expect(
      resolveEngineConnectionState({ status: status(true), connecting: false, failed: true })
    ).toBe("connected");
    expect(
      resolveEngineConnectionState({ status: status(false), connecting: false, failed: true })
    ).toBe("failed");
  });

  it("falls back to idle without a handshake", () => {
    expect(
      resolveEngineConnectionState({ status: undefined, connecting: false, failed: false })
    ).toBe("idle");
    expect(hasHandshake(status(false))).toBe(false);
    expect(hasHandshake(status(true))).toBe(true);
  });

  it("labels every state distinctly", () => {
    const states = (["idle", "connecting", "connected", "failed"] as const).map(
      (state) => engineConnectionLabel(state, "Codex").zh
    );
    expect(new Set(states).size).toBe(states.length);
  });
});
