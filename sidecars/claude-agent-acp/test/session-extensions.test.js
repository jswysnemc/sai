import assert from "node:assert/strict";
import test from "node:test";

import {
  applySaiSessionExtensions,
  withSaiSkillsSystemPrompt,
} from "../src/session-extensions.js";

/** 构造带有 Sai Skills 的 ACP 会话参数。 */
function sessionParams() {
  return {
    cwd: "/workspace",
    _meta: {
      traceId: "trace-1",
      _sai: {
        skills: "Use the repository skills.",
        workspace: "sai",
      },
    },
  };
}

test("converts Sai Skills without losing metadata", () => {
  const params = sessionParams();
  const converted = withSaiSkillsSystemPrompt(params);

  assert.notStrictEqual(converted, params);
  assert.equal(converted._meta.traceId, "trace-1");
  assert.deepEqual(converted._meta._sai, params._meta._sai);
  assert.deepEqual(converted._meta.systemPrompt, {
    type: "preset",
    preset: "claude_code",
    append: "Use the repository skills.",
  });
  assert.equal(params._meta.systemPrompt, undefined);
});

test("preserves an explicit system prompt", () => {
  const params = sessionParams();
  params._meta.systemPrompt = "Custom system prompt";

  assert.strictEqual(withSaiSkillsSystemPrompt(params), params);
});

test("wraps every supported session entry and preserves this", async () => {
  const methodNames = [
    "newSession",
    "loadSession",
    "resumeSession",
    "unstable_forkSession",
  ];
  const agent = { marker: "agent" };
  for (const methodName of methodNames) {
    agent[methodName] = function sessionMethod(params) {
      assert.equal(this.marker, "agent");
      return params;
    };
  }

  applySaiSessionExtensions(agent);
  for (const methodName of methodNames) {
    const converted = await agent[methodName](sessionParams());
    assert.equal(converted._meta.systemPrompt.append, "Use the repository skills.");
  }
});
