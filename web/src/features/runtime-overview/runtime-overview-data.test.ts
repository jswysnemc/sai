import { describe, expect, it } from "vitest";
import {
  countGitPatchLines,
  normalizeTodoItems,
  selectTodoOverviewItems,
  sortSubagentOverviewItems
} from "./runtime-overview-data";
import type { Subagent, TodoItem } from "../../api/contracts";

describe("runtime overview data", () => {
  it("counts changed lines without diff headers", () => {
    const patch = [
      "--- a/file.ts",
      "+++ b/file.ts",
      "@@ -1,2 +1,3 @@",
      "-old",
      "+new",
      "+added"
    ].join("\n");

    expect(countGitPatchLines(patch)).toEqual({ added: 2, removed: 1 });
  });

  it("normalizes current and legacy todo responses", () => {
    const item = { id: "todo-1", text: "检查结果", status: "pending" as const, created_at: "", updated_at: "" };

    expect(normalizeTodoItems([item])).toEqual([item]);
    expect(normalizeTodoItems({ items: [item], history: [] })).toEqual([item]);
  });

  it("keeps every plan item available in the overview", () => {
    const items: TodoItem[] = Array.from({ length: 6 }, (_, index) => ({
      id: `todo-${index}`,
      text: `计划 ${index + 1}`,
      status: "pending",
      created_at: "",
      updated_at: ""
    }));

    expect(selectTodoOverviewItems(items).map((item) => item.id)).toEqual(
      items.map((item) => item.id)
    );
  });

  it("shows every subagent with running work first and latest finished tasks next", () => {
    const subagent = (
      id: string,
      status: string,
      updatedAt: number
    ): Subagent => ({
      id,
      description: id,
      subagent_type: "general",
      status,
      max_steps: 0,
      started_at: updatedAt - 1,
      updated_at: updatedAt,
      step: 0
    });
    const items = [
      subagent("old", "completed", 1),
      subagent("latest", "completed", 4),
      subagent("running", "running", 2),
      subagent("middle", "completed", 3)
    ];

    expect(sortSubagentOverviewItems(items).map((item) => item.id)).toEqual([
      "running",
      "latest",
      "middle",
      "old"
    ]);
  });
});
