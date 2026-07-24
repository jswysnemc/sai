import { describe, expect, it } from "vitest";
import { isAbsoluteFilesystemPath } from "./path-utils";

describe("isAbsoluteFilesystemPath", () => {
  it("accepts posix absolute paths", () => {
    expect(isAbsoluteFilesystemPath("/home/user")).toBe(true);
  });

  it("accepts windows drive and unc paths", () => {
    expect(isAbsoluteFilesystemPath("C:\\Users\\demo")).toBe(true);
    expect(isAbsoluteFilesystemPath("c:/Users/demo")).toBe(true);
    expect(isAbsoluteFilesystemPath("\\\\server\\share")).toBe(true);
    expect(isAbsoluteFilesystemPath("//server/share")).toBe(true);
  });

  it("rejects relative paths", () => {
    expect(isAbsoluteFilesystemPath("Users\\demo")).toBe(false);
    expect(isAbsoluteFilesystemPath("src/app")).toBe(false);
  });
});
