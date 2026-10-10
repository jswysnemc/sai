import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { I18nContext } from "../../i18n/i18n-context";
import { text, type Locale } from "../../i18n/locale";
import { JevToolDetail } from "./jev-resource-detail";

const longName = "historical_context_block_pagination_cursor_including_workspace_revision";

/**
 * 用指定界面语言渲染工具详情。
 *
 * @param locale 界面语言
 * @param shownMeta 折叠行已经展示的说明
 * @returns 静态 HTML
 */
function renderDetail(locale: Locale, shownMeta: string): string {
  return renderToStaticMarkup(
    <I18nContext.Provider value={{ locale, setLocale: () => undefined, t: (en, zh) => text(locale, en, zh) }}>
      <JevToolDetail
        shownMeta={shownMeta}
        item={{
          kind: "tool",
          name: "context_status",
          detail: "Inspect the current context blocks.",
          description: "Inspect the current context blocks. Then continue.",
          parameters: [{ name: longName, type: "integer", required: false, description: "" }]
        }}
      />
    </I18nContext.Provider>
  );
}

describe("JevToolDetail", () => {
  it("标明参数表是暴露的 schema，长参数名放在可换行的 code 里", () => {
    const html = renderDetail("zh-CN", "");
    expect(html).toContain("暴露的参数，不是本次调用参数");
    expect(html).toContain(`<code>${longName}</code>`);
    expect(html).toContain("jev-resource-param");
    expect(html).not.toContain("Inspect the current context blocks.");
    expect(html).not.toContain("Then continue.");
  });

  it("英文界面展开后不再重复折叠行已经显示的首句", () => {
    const html = renderDetail("en-US", "Inspect the current context blocks.");
    expect(html).toContain("Exposed schema, not this call");
    expect(html).toContain("Then continue.");
    expect(html).not.toContain("Inspect the current context blocks.");
  });
});
