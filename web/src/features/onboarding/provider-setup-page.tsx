import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import type { ConfigResponse } from "../../api/contracts";
import { completeProviderSetup } from "../../api/onboarding";
import { useI18n } from "../i18n/use-i18n";
import { InlineNotice } from "../settings/kit";
import { createProviderSetupDraft, providerSetupInput, providerSetupIssue } from "./provider-setup-draft";
import { ProviderSetupForm } from "./provider-setup-form";

/**
 * 【首次配置】【引导页面】管理配置草稿和提交状态，保存成功后更新共享缓存并进入原页面。
 * @param props 当前脱敏配置响应
 * @returns 首次使用页面
 */
export function ProviderSetupPage({ response }: { response: ConfigResponse }) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const providers = response.config.providers;
  const [draft, setDraft] = useState(() => createProviderSetupDraft(
    providers.find(provider => provider.id === response.config.active_provider) ?? providers[0], response.secret_sentinel
  ));
  const [validation, setValidation] = useState<string | null>(null);
  const save = useMutation({
    mutationFn: completeProviderSetup,
    onSuccess: saved => {
      queryClient.removeQueries({ queryKey: ["engine-status"] });
      queryClient.setQueryData(["config"], saved);
    }
  });

  /**
   * 【首次配置】【提交校验】校验基本字段后保存，错误期间保留用户输入。
   * @returns 无
   */
  const submit = () => {
    if (save.isPending) return;
    const issue = providerSetupIssue(draft);
    const messages = {
      name: t("Enter a provider name.", "请填写供应商名称。"),
      url: t("Enter a valid HTTP(S) API address.", "请填写有效的 HTTP(S) API 地址。"),
      model: t("Enter a default model.", "请填写默认模型。")
    };
    setValidation(issue ? messages[issue] : null);
    if (!issue) save.mutate(providerSetupInput(draft));
  };

  return (
    <section className="mx-auto flex min-h-full w-full max-w-[38rem] flex-col justify-center gap-5 px-4 py-6 text-[0.875rem] sm:px-6 sm:py-10" aria-labelledby="provider-setup-title">
      <header className="space-y-2">
        <p className="text-[0.75rem] text-[var(--ink-soft)]">{t("Welcome to Sai", "欢迎使用 Sai")}</p>
        <h1 id="provider-setup-title" className="text-[1.125rem] font-semibold sm:text-[1.25rem]">{t("Set up your first provider", "配置首个供应商")}</h1>
        <p className="text-[var(--ink-soft)]">{t("Choose a provider and default model before your first conversation. This setup is shared by the terminal and Web workbench.", "开始首次对话前，请选择供应商与默认模型。终端和 Web 工作台共用此配置。")}</p>
      </header>
      <div className="space-y-4 rounded-[var(--radius-3)] border border-[var(--line)] p-4 sm:p-5">
        {(validation || save.error) && <InlineNotice tone="danger">{validation ?? save.error?.message}</InlineNotice>}
        <ProviderSetupForm providers={providers} draft={draft} saving={save.isPending}
          onSelect={id => {
            setDraft(createProviderSetupDraft(providers.find(provider => provider.id === id), response.secret_sentinel));
            setValidation(null);
            save.reset();
          }}
          onChange={patch => { setDraft(current => ({ ...current, ...patch })); setValidation(null); save.reset(); }}
          onSubmit={submit}
        />
      </div>
      <p className="text-[0.75rem] text-[var(--ink-soft)]">{t("You can change providers and models later in Settings.", "以后可以在设置中修改供应商和模型。")}</p>
    </section>
  );
}
