import { describe, expect, it } from "vitest";
import {
  clearComposerAttachmentDraft,
  readComposerAttachmentDraft,
  writeComposerAttachmentDraft
} from "./composer-attachments-draft";

describe("composer attachment draft", () => {
  it("keeps attachments across clear of another session", () => {
    writeComposerAttachmentDraft("session-a", [{ id: 1, name: "a.png", dataUrl: "data:image/png;base64,AA==" }]);
    writeComposerAttachmentDraft("session-b", [{ id: 2, name: "b.png", dataUrl: "data:image/png;base64,BB==" }]);
    clearComposerAttachmentDraft("session-b");
    expect(readComposerAttachmentDraft("session-a")).toEqual([
      { id: 1, name: "a.png", dataUrl: "data:image/png;base64,AA==" }
    ]);
    expect(readComposerAttachmentDraft("session-b")).toEqual([]);
  });
});
