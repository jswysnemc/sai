import { beforeEach, describe, expect, it, vi } from "vitest";
import { apiRequest } from "../../api/api-request";
import { templateApi, templateKey } from "./template-client";
import { filterSkills } from "../chat/composer/skill-mention-popover";

vi.mock("../../api/api-request", () => ({ apiRequest: vi.fn() }));
beforeEach(() => vi.clearAllMocks());

describe("input template persistence", () => {
  it("uses separate server routes and cache keys for chat and image templates", async () => {
    vi.mocked(apiRequest).mockResolvedValue({ items: [] });
    await templateApi.list("chat");
    await templateApi.list("image");
    expect(apiRequest).toHaveBeenNthCalledWith(1, "/api/prompts/chat-templates");
    expect(apiRequest).toHaveBeenNthCalledWith(2, "/api/prompts/image-templates");
    expect(templateKey("chat")).not.toEqual(templateKey("image"));
  });
  it("creates and renames with the original keyword in the update route", async () => {
    const draft = { name: "new-name", content: "first\nsecond\n" };
    await templateApi.save("image", draft);
    await templateApi.save("image", draft, "old-name");
    expect(apiRequest).toHaveBeenNthCalledWith(1, "/api/prompts/image-templates", { method: "POST", body: JSON.stringify(draft) });
    expect(apiRequest).toHaveBeenNthCalledWith(2, "/api/prompts/image-templates/old-name", { method: "PUT", body: JSON.stringify(draft) });
  });
  it("loads complete templates in a single request", async () => {
    const items = [{ name: "review", content: "Review code", builtin: false }];
    vi.mocked(apiRequest).mockResolvedValue({ items });
    expect(await templateApi.list("chat")).toEqual(items);
    expect(apiRequest).toHaveBeenCalledTimes(1);
  });
  it("matches template keywords and descriptions without conflating skill identifiers", () => {
    const options = [
      { name: "review", description: "Skill" },
      { name: "template:review", keyword: "review", description: "检查代码", kind: "template" as const }
    ];
    expect(filterSkills(options, "REVIEW")).toHaveLength(2);
    expect(filterSkills(options, "检查")).toEqual([options[1]]);
    expect(filterSkills(options, "template:")).toEqual([]);
  });
});
