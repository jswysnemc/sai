import { describe, expect, it } from "vitest";
import type { GitCommitSummary } from "../../../api/contracts";
import { calculateGitGraphLanes } from "./graph-lanes";

describe("calculateGitGraphLanes", () => {
  it("keeps a linear history on one lane", () => {
    const rows = calculateGitGraphLanes([
      commit("a", ["b"]),
      commit("b", ["c"]),
      commit("c", [])
    ]);

    expect(rows.map((row) => row.nodeLane)).toEqual([0, 0, 0]);
    expect(rows.map((row) => row.laneCount)).toEqual([1, 1, 1]);
    expect(rows[1].segments).toContainEqual({
      fromLane: 0,
      fromPoint: "top",
      toLane: 0,
      toPoint: "node",
      color: 0
    });
  });

  it("renders merge parents and joins the second branch", () => {
    const rows = calculateGitGraphLanes([
      commit("merge", ["left", "right"]),
      commit("left", ["base"]),
      commit("right", ["base"]),
      commit("base", [])
    ]);

    expect(rows[0].laneCount).toBe(2);
    expect(rows[0].segments.filter((segment) => segment.fromPoint === "node")).toHaveLength(2);
    expect(rows[1].laneCount).toBe(2);
    expect(rows[2].nodeLane).toBe(1);
    expect(rows[2].segments).toContainEqual(expect.objectContaining({
      fromLane: 1,
      fromPoint: "node",
      toLane: 0,
      toPoint: "bottom"
    }));
    expect(rows[3].laneCount).toBe(1);
  });

  it("starts an independent tip and connects it to an active ancestor", () => {
    const rows = calculateGitGraphLanes([
      commit("local", ["base"]),
      commit("remote", ["base"]),
      commit("base", [])
    ]);

    expect(rows[1].nodeLane).toBe(1);
    expect(rows[1].segments).toContainEqual(expect.objectContaining({
      fromLane: 1,
      fromPoint: "node",
      toLane: 0,
      toPoint: "bottom"
    }));
  });

  it("keeps a parent lane open when history is truncated", () => {
    const [row] = calculateGitGraphLanes([commit("visible", ["not-loaded"])]);

    expect(row.segments).toContainEqual(expect.objectContaining({
      fromPoint: "node",
      toPoint: "bottom"
    }));
    expect(row.laneCount).toBe(1);
  });
});

/**
 * 创建轨道算法所需的最小提交摘要。
 *
 * @param sha 提交标识
 * @param parents 父提交标识
 * @returns 完整提交摘要
 */
function commit(sha: string, parents: string[]): GitCommitSummary {
  return {
    sha,
    short_sha: sha,
    parents,
    refs: [],
    subject: sha,
    author_name: "Test",
    author_email: "test@example.com",
    author_date: "2026-01-01T00:00:00Z",
    files: [],
    file_count: 0,
    local_only: false,
    remote_only: false
  };
}
