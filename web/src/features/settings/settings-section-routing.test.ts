import { describe, expect, it } from "vitest";
import { SETTINGS_SECTIONS } from "./settings-registry";
import {
  APP_CONFIG_SECTION_IDS,
  STANDALONE_SECTION_IDS,
  isAppConfigSection
} from "./settings-section-routing";

describe("settings section routing", () => {
  it("routes every registered section to exactly one renderer", () => {
    // 分区只登记在注册表而漏掉渲染分支时，界面会渲染成空白
    const routed = new Set<string>([...APP_CONFIG_SECTION_IDS, ...STANDALONE_SECTION_IDS]);
    for (const section of SETTINGS_SECTIONS) {
      expect(routed.has(section.id), `${section.id} has no render branch`).toBe(true);
    }
    expect(routed.size).toBe(APP_CONFIG_SECTION_IDS.length + STANDALONE_SECTION_IDS.length);
  });

  it("does not route sections that are not registered", () => {
    const registered = new Set<string>(SETTINGS_SECTIONS.map((section) => section.id));
    for (const id of [...APP_CONFIG_SECTION_IDS, ...STANDALONE_SECTION_IDS]) {
      expect(registered.has(id), `${id} is routed but not registered`).toBe(true);
    }
  });

  it("matches the appConfig requirement declared in the registry", () => {
    // required 的分区必须走 AppConfig 分支，否则拿不到已加载的配置
    for (const section of SETTINGS_SECTIONS) {
      expect(isAppConfigSection(section.id), `${section.id} routing does not match appConfig`).toBe(
        section.appConfig === "required"
      );
    }
  });

  it("keeps ssh on the standalone branch", () => {
    // SSH 自带主机增删改查接口，不依赖全局 AppConfig 加载
    expect(isAppConfigSection("ssh")).toBe(false);
    expect(STANDALONE_SECTION_IDS).toContain("ssh");
  });
});
