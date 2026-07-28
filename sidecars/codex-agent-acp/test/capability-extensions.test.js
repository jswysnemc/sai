import assert from "node:assert/strict";
import test from "node:test";

import {
  SAI_CAPABILITIES,
  extendInitializeResponse,
  trackInitializeRequest,
} from "../src/capability-extensions.js";

test("extends only the matching initialize response", () => {
  const ids = new Set();
  const request = JSON.stringify({ jsonrpc: "2.0", id: 7, method: "initialize", params: {} });
  trackInitializeRequest(request, ids);

  const response = JSON.stringify({
    jsonrpc: "2.0",
    id: 7,
    result: {
      protocolVersion: 1,
      _meta: {
        steering: { supported: true },
        _sai: { native_equivalents: { steering: "codex" } },
      },
    },
  });
  const extended = JSON.parse(extendInitializeResponse(response, ids));

  assert.deepEqual(extended.result._meta.steering, { supported: true });
  assert.deepEqual(extended.result._meta._sai.capabilities, SAI_CAPABILITIES);
  assert.equal(extended.result._meta._sai.native_equivalents.subagents, "codex");
  assert.equal(extended.result._meta._sai.native_equivalents.steering, "codex");
  assert.equal(ids.size, 0);
});

test("preserves unmatched and invalid JSONL lines", () => {
  const ids = new Set(["number:1"]);
  const notification = JSON.stringify({ jsonrpc: "2.0", method: "session/update", params: {} });

  assert.equal(extendInitializeResponse(notification, ids), notification);
  assert.equal(extendInitializeResponse("not-json", ids), "not-json");
  assert.equal(trackInitializeRequest("not-json", ids), "not-json");
});
