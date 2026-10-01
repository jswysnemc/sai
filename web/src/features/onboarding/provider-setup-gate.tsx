import { lazy, type ReactNode } from "react";
import { useQuery } from "@tanstack/react-query";
import { api } from "../../api/client";
import { Button } from "../../shared/ui/button/button";
import { LoadingPanel } from "../../shared/ui/loading-panel";
import { useI18n } from "../i18n/use-i18n";
import { needsProviderSetup } from "./provider-setup-draft";

const ProviderSetupPage = lazy(() => import("./provider-setup-page").then(module => ({ default: module.ProviderSetupPage })));

/**
 * 【首次配置】【网页入口】认证后先读取共享状态，未完成时呈现引导并保留原访问路由。
 * @param props 原页面内容
 * @returns 引导、加载状态或原页面
 */
export function ProviderSetupGate({ children }: { children: ReactNode }) {
  const { t } = useI18n();
  const query = useQuery({ queryKey: ["config"], queryFn: api.config.load });
  if (query.isPending) return <LoadingPanel />;
  // 【首次配置】【缓存保留】后台刷新失败时继续使用已确认状态，避免卸载正在编辑的工作台
  if (!query.data) return (
    <div className="mx-auto flex max-w-[32rem] flex-col gap-3 p-6 text-[0.875rem]" role="alert">
      <p>{t("Could not load provider settings.", "无法读取供应商配置。")}</p>
      <p>{query.error?.message}</p>
      <Button onClick={() => void query.refetch()}>{t("Retry", "重试")}</Button>
    </div>
  );
  if (needsProviderSetup(query.data.config)) return <ProviderSetupPage response={query.data} />;
  return children;
}
