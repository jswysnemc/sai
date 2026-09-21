import { describe, expect, it } from "vitest";
import type { DirectoryEntry } from "../../api/contracts";
import { dateFromDirectoryName, sortDirectoryEntries } from "./directory-sorting";

function directory(name: string): DirectoryEntry {
  return { name, path: `/workspace/${name}`, git_repository: false };
}

describe("directory sorting", () => {
  it("sorts date named directories newest first", () => {
    const result = sortDirectoryEntries([
      directory("notes"),
      directory("2024-12-31"),
      directory("2025_01_02"),
      directory("20240101")
    ]);

    expect(result.map((entry) => entry.name)).toEqual(["2025_01_02", "2024-12-31", "20240101", "notes"]);
  });

  it("keeps natural ordering for non-date directories and hides dot folders", () => {
    const result = sortDirectoryEntries([directory("item10"), directory(".cache"), directory("item2"), directory("item1")]);

    expect(result.map((entry) => entry.name)).toEqual(["item1", "item2", "item10", ".cache"]);
  });

  it("rejects impossible calendar dates", () => {
    expect(dateFromDirectoryName("2025-02-31")).toBeNull();
    expect(dateFromDirectoryName("2025-02")).not.toBeNull();
  });
});
