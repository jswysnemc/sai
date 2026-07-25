import { describe, expect, it } from "vitest";
import { collectTurnFileChanges } from "./collect-turn-file-changes";

describe("collectTurnFileChanges", () => {
  it("merges edit tool outputs", () => {
    const changes = collectTurnFileChanges([
      {
        name: "write_file",
        status: "completed",
        output: JSON.stringify({
          ok: true,
          changed_files: [{ action: "Added", path: "a.ts", added: 3, removed: 0 }]
        })
      },
      {
        name: "str_replace",
        status: "completed",
        output: JSON.stringify({
          ok: true,
          changed_files: [{ action: "Edited", path: "b.ts", added: 1, removed: 1 }]
        })
      },
      {
        name: "edit_file",
        status: "completed",
        output: JSON.stringify({
          ok: true,
          changed_files: [{ action: "Edited", path: "a.ts", added: 2, removed: 1 }]
        })
      }
    ]);
    expect(changes).toEqual([
      { path: "a.ts", action: "Added", added: 5, removed: 1, tool: "write_file" },
      { path: "b.ts", action: "Edited", added: 1, removed: 1, tool: "str_replace" }
    ]);
  });
});
