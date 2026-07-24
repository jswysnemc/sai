import { describe, expect, it } from "vitest";
import { ComposerEditHistory, isRedoShortcut, isUndoShortcut } from "./composer-edit-history";

describe("composer edit history", () => {
  it("undoes and redoes text snapshots", () => {
    const history = new ComposerEditHistory();
    history.reset({ value: "", selection: { start: 0, end: 0 } });
    history.record(
      { value: "", selection: { start: 0, end: 0 } },
      { value: "ab", selection: { start: 2, end: 2 } }
    );
    history.record(
      { value: "ab", selection: { start: 2, end: 2 } },
      { value: "abcd", selection: { start: 4, end: 4 } }
    );

    const undone = history.undo({ value: "abcd", selection: { start: 4, end: 4 } });
    expect(undone).toEqual({ value: "ab", selection: { start: 2, end: 2 } });
    const redone = history.redo({ value: "ab", selection: { start: 2, end: 2 } });
    expect(redone).toEqual({ value: "abcd", selection: { start: 4, end: 4 } });
  });

  it("detects undo and redo shortcuts", () => {
    expect(isUndoShortcut({ key: "z", ctrlKey: true, metaKey: false, shiftKey: false, altKey: false })).toBe(true);
    expect(isUndoShortcut({ key: "z", ctrlKey: true, metaKey: false, shiftKey: true, altKey: false })).toBe(false);
    expect(isRedoShortcut({ key: "y", ctrlKey: true, metaKey: false, shiftKey: false, altKey: false })).toBe(true);
    expect(isRedoShortcut({ key: "z", ctrlKey: true, metaKey: false, shiftKey: true, altKey: false })).toBe(true);
  });
});
