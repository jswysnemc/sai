import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderToStaticMarkup } from "react-dom/server";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";
import type { AppConfig } from "../../../api/contracts";
import { DialogProvider } from "../../../shared/ui/dialog/dialog-provider";
import { JevSettingsSection } from "./jev-settings-section";

describe("Jev settings workbench", () => {
  it("keeps connection management and both feature groups reachable without a subview", () => {
    const config = { providers: [], gateways: {}, active_provider: "", model_endpoints: [] } as unknown as AppConfig;
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const html = renderToStaticMarkup(<MemoryRouter><QueryClientProvider client={client}><DialogProvider>
      <JevSettingsSection config={config} secretSentinel="hidden" dirty={false} onConfigChange={() => undefined} />
    </DialogProvider></QueryClientProvider></MemoryRouter>);
    for (const label of ["启用暴露决策", "启用 Jev 审核", "管理接入", "新增模型接入", "最低概率", "最低置信度"]) expect(html).toContain(label);
  });
});
