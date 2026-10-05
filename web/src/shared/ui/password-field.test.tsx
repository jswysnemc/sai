import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { isExternalValueChange, PasswordField } from "./password-field";
import { SkSecretInput } from "../../features/settings/kit/secret-input";

/*
 * 覆盖范围的说明，避免把这里当成完整的行为测试：
 *
 * 项目没有 jsdom / @testing-library/react，renderToStaticMarkup 每次都是全新挂载，
 * 既模拟不了点击，也模拟不了带新 props 的重渲染。因此下面钉住的是
 * isExternalValueChange 的判定规则（取值由外部改写时必须收起明文、
 * 用户自己打字时不能收起）和各状态的静态渲染结果，
 * 而不是「切换供应商触发重挂载」这个动作本身——去掉父级的 key={provider.id}
 * 本文件照样会过。真正的交互级验证需要引入测试运行时，目前不做。
 */

describe("PasswordField", () => {
  it("设置页的已保存密钥直接显示圆点掩码，不需要先点编辑", () => {
    const onChange = vi.fn();
    const html = renderToStaticMarkup(
      <SkSecretInput value="saved-sentinel" secretSentinel="saved-sentinel" onChange={onChange} onReveal={async () => "fixture-secret"} />
    );
    expect(html).toContain('type="password"');
    expect(html).toContain("••••");
    expect(html).not.toContain("编辑");
    expect(html).not.toContain("saved-sentinel");
    expect(html).not.toContain("fixture-secret");
    expect(onChange).not.toHaveBeenCalled();
  });

  it("设置页密钥输入框关闭自动填充，避免被识别为登录密码", () => {
    const html = renderToStaticMarkup(<SkSecretInput value="" onChange={vi.fn()} />);
    expect(html).toContain('autoComplete="new-password"');
    expect(html).toContain('data-1p-ignore="true"');
  });

  it("默认以掩码态渲染", () => {
    const html = renderToStaticMarkup(<PasswordField value="sk-provider-a" onChange={vi.fn()} />);

    expect(html).toContain('type="password"');
    expect(html).not.toContain('type="text"');
  });

  it("切换到另一个供应商后回到掩码态", () => {
    // 供应商切换由父级的 key={provider.id} 触发重建，等价于一次全新挂载
    const switched = renderToStaticMarkup(<PasswordField value="sk-provider-b" onChange={vi.fn()} />);

    expect(switched).toContain('type="password"');
    expect(switched).not.toContain('type="text"');
    // 复用实例时靠这条规则收起明文，否则上一个供应商的密钥会直接露出来
    expect(isExternalValueChange("sk-provider-b", "sk-provider-a", null)).toBe(true);
  });

  it("已保存的敏感值在空输入框里带标记，与未设置区分开", () => {
    const html = renderToStaticMarkup(
      <PasswordField value="" savedValueHint="已保存" onClearSavedValue={vi.fn()} onChange={vi.fn()} />
    );

    expect(html).toContain("ui-password-field-clear");
    expect(html).toContain("清除已保存的值");
  });
});

describe("isExternalValueChange", () => {
  it("取值未变时保持当前状态", () => {
    expect(isExternalValueChange("sk-a", "sk-a", null)).toBe(false);
  });

  it("用户在明文态继续输入时不收起", () => {
    expect(isExternalValueChange("sk-a-new", "sk-a", "sk-a-new")).toBe(false);
  });

  it("取值被外部改写时收起明文", () => {
    expect(isExternalValueChange("sk-b", "sk-a", null)).toBe(true);
  });
});
