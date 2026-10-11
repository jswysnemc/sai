import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { DialogProvider } from "../../../shared/ui/dialog/dialog-provider";
import { SettingsSaveBar } from "./settings-save-bar";
import { Save } from "../../../shared/ui/icons";
import type { SettingsSectionMeta } from "../settings-types";

const mockMeta: SettingsSectionMeta = {
  id: "runtime",
  group: "basics",
  appConfig: "required",
  labelEn: "Runtime",
  labelZh: "运行时",
  descriptionEn: "Runtime settings",
  descriptionZh: "运行时设置",
  icon: Save,
  searchKeys: ["runtime"]
};

describe("SettingsSaveBar", () => {
  it("无修改且已加载时不渲染悬浮栏", () => {
    const html = renderToStaticMarkup(
      <DialogProvider>
        <SettingsSaveBar
          meta={mockMeta}
          dirty={false}
          saving={false}
          loaded={true}
          onSave={vi.fn()}
          onDiscard={vi.fn()}
        />
      </DialogProvider>
    );
    expect(html).not.toContain("settings-save-dock");
  });

  it("有待保存修改时显示状态和保存按钮", () => {
    const html = renderToStaticMarkup(
      <DialogProvider>
        <SettingsSaveBar
          meta={mockMeta}
          dirty={true}
          saving={false}
          loaded={true}
          onSave={vi.fn()}
          onDiscard={vi.fn()}
        />
      </DialogProvider>
    );

    expect(html).toContain("settings-save-dock");
    expect(html).toContain("有未保存的修改");
    expect(html).toContain("放弃");
    expect(html).toContain("保存");
  });

  it("保存中状态禁用按钮并展示正在保存文案", () => {
    const html = renderToStaticMarkup(
      <DialogProvider>
        <SettingsSaveBar
          meta={mockMeta}
          dirty={true}
          saving={true}
          loaded={true}
          onSave={vi.fn()}
          onDiscard={vi.fn()}
        />
      </DialogProvider>
    );

    expect(html).toContain("正在保存");
    expect(html).toContain("disabled");
  });

  it("存在校验错误时显示提示并禁用保存", () => {
    const html = renderToStaticMarkup(
      <DialogProvider>
        <SettingsSaveBar
          meta={mockMeta}
          dirty={true}
          saving={false}
          loaded={true}
          validationError="Invalid JSON format"
          onSave={vi.fn()}
          onDiscard={vi.fn()}
        />
      </DialogProvider>
    );

    expect(html).toContain("请先修正 JSON");
    expect(html).toContain("disabled");
  });
});
