import { lazy } from "react";
import { Navigate, Route, Routes } from "react-router-dom";
import { AppShell } from "./app-shell";
import { ChatAgentProvider } from "../features/agents/chat-agent-context";
import { DialogProvider } from "../shared/ui/dialog/dialog-provider";
import { I18nProvider } from "../features/i18n/i18n-context";

// 1. 【前端性能】【路由加载】页面模块在对应路由激活时加载
const CodingPage = lazy(() => import("../features/workspace/coding-page").then((module) => ({ default: module.CodingPage })));
const SettingsPage = lazy(() => import("../features/settings/settings-page").then((module) => ({ default: module.SettingsPage })));
const GatewaysPage = lazy(() => import("../features/gateways/gateways-page").then((module) => ({ default: module.GatewaysPage })));
const CronJobsPage = lazy(() => import("../features/cron-jobs/cron-jobs-page").then((module) => ({ default: module.CronJobsPage })));
const ImageWorkbenchPage = lazy(() => import("../features/image-workbench/image-workbench-page").then((module) => ({ default: module.ImageWorkbenchPage })));

/**
 * 组合应用级上下文和页面路由。
 *
 * @returns 应用入口
 */
export function App() {
  return (
    <I18nProvider>
      <DialogProvider>
        <ChatAgentProvider>
          <Routes>
            <Route element={<AppShell />}>
              <Route index element={<CodingPage />} />
              <Route path="settings" element={<Navigate to="/settings/providers" replace />} />
              <Route path="settings/:sectionId/:subview?" element={<SettingsPage />} />
              <Route path="gateways" element={<GatewaysPage />} />
              <Route path="cron-jobs" element={<CronJobsPage />} />
              <Route path="image-workbench" element={<ImageWorkbenchPage />} />
              <Route path="*" element={<Navigate to="/" replace />} />
            </Route>
          </Routes>
        </ChatAgentProvider>
      </DialogProvider>
    </I18nProvider>
  );
}
