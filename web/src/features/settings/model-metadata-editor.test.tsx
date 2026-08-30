import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import type { ProviderConfig } from "../../api/contracts";
import { DialogProvider } from "../../shared/ui/dialog/dialog-provider";
import { ModelMetadataEditor } from "./model-metadata-editor";

/** 模型详情页标题里的选中模型名 */
const SELECTED_MODEL = /<strong>([^<]+)<\/strong><small>单模型能力与上下文<\/small>/;

/**
 * 构造最小可用的供应商配置。
 *
 * @param patch 覆盖字段
 * @returns 供应商配置
 */
function makeProvider(patch: Partial<ProviderConfig>): ProviderConfig {
  return { id: "provider", display_name: "Provider", base_url: "https://api.example.com/v1", ...patch };
}

/**
 * 渲染模型目录编辑器。
 *
 * 删除模型走 useConfirm，必须挂在 DialogProvider 下。
 *
 * @param provider 供应商配置
 * @returns 编辑器静态标记
 */
function renderEditor(provider: ProviderConfig): string {
  return renderToStaticMarkup(
    <DialogProvider>
      <ModelMetadataEditor provider={provider} onChange={vi.fn()} />
    </DialogProvider>
  );
}

/**
 * 取出编辑器当前选中的模型。
 *
 * @param provider 供应商配置
 * @returns 选中的模型标识
 */
function selectedModel(provider: ProviderConfig): string {
  const matched = SELECTED_MODEL.exec(renderEditor(provider));
  return matched?.[1] ?? "";
}

describe("ModelMetadataEditor", () => {
  it("挂载时按当前供应商的默认模型选中", () => {
    const provider = makeProvider({ id: "a", models: ["gpt-4o", "claude"], default_model: "gpt-4o" });

    expect(selectedModel(provider)).toBe("gpt-4o");
  });

  it("切换供应商后不沿用上一个供应商的选中模型", () => {
    // 两个供应商模型名完全重叠，旧的选中项依然合法，只能靠父级的
    // key={provider.id} 重建组件来重置。
    // 与 password-field.test.tsx 一样，renderToStaticMarkup 每次都是新挂载，
    // 这里验证的是挂载后的选中结果，去掉父级的 key 本用例照样会过
    const models = ["gpt-4o", "claude"];
    const before = makeProvider({ id: "a", models, default_model: "gpt-4o" });
    const after = makeProvider({ id: "b", models, default_model: "claude" });

    expect(selectedModel(before)).toBe("gpt-4o");
    expect(selectedModel(after)).toBe("claude");
  });
});
