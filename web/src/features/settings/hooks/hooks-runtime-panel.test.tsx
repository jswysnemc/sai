import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { HooksRuntimePanel } from "./hooks-runtime-panel";

const mocks = vi.hoisted(() => ({ request: vi.fn(), confirm: vi.fn() }));
vi.mock("../../../api/api-request", () => ({ apiRequest: mocks.request }));
vi.mock("../../../shared/ui/dialog/dialog-provider", () => ({ useConfirm: () => mocks.confirm }));
vi.mock("../../i18n/use-i18n", () => ({ useI18n: () => ({ t: (en: string) => en }) }));

describe("hooks runtime panel", () => {
  it("explains optional sources without executing or authorizing hooks during rendering", () => {
    const html = renderToStaticMarkup(<HooksRuntimePanel />);
    expect(html).toContain("Sources and recent executions");
    expect(html).toContain("project .sai/hooks.json");
    expect(html).not.toContain("<details");
    expect(html).not.toContain("<summary");
    expect(mocks.request).not.toHaveBeenCalled();
    expect(mocks.confirm).not.toHaveBeenCalled();
  });
});
