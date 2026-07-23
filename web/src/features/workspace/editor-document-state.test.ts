import { describe, expect, it } from "vitest";
import type { FileContent } from "../../api/contracts";
import {
  applyRemoteFile,
  canSaveDocument,
  createEditorDocumentState,
  reloadRemoteFile,
  updateDocumentContent
} from "./editor-document-state";

/**
 * 构造编辑器文件快照。
 *
 * @param version 内容版本
 * @param content 文件内容
 * @returns 文件快照
 */
function file(version: string, content: string): FileContent {
  return { path: "src/main.rs", content, size: content.length, version, modified_at: 1 };
}

describe("editor document state", () => {
  it("keeps the original save baseline after an external refresh", () => {
    let state = applyRemoteFile(createEditorDocumentState("src/main.rs"), file("v1", "one"));
    state = updateDocumentContent(state, "draft");
    state = applyRemoteFile(state, file("v2", "external"));

    expect(state.content).toBe("draft");
    expect(state.baseline?.version).toBe("v1");
    expect(state.externalChange).toBe(true);
    expect(canSaveDocument(state)).toBe(false);
  });

  it("reloads the latest remote snapshot on explicit request", () => {
    let state = applyRemoteFile(createEditorDocumentState("src/main.rs"), file("v1", "one"));
    state = updateDocumentContent(state, "draft");
    state = applyRemoteFile(state, file("v2", "external"));

    state = reloadRemoteFile(state);

    expect(state.content).toBe("external");
    expect(state.baseline?.version).toBe("v2");
    expect(state.externalChange).toBe(false);
  });

  it("adopts remote refreshes while the document is clean", () => {
    let state = applyRemoteFile(createEditorDocumentState("src/main.rs"), file("v1", "one"));

    state = applyRemoteFile(state, file("v2", "two"));

    expect(state.content).toBe("two");
    expect(state.baseline?.version).toBe("v2");
  });

  it("treats clearing a non-empty file as a dirty draft", () => {
    let state = applyRemoteFile(createEditorDocumentState("src/main.rs"), file("v1", "one"));

    state = updateDocumentContent(state, "");

    expect(canSaveDocument(state)).toBe(true);
  });
});
