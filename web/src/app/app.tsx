import { Navigate, Route, Routes } from "react-router-dom";
import { AppShell } from "./app-shell";
import { GatewaysPage } from "../features/gateways/gateways-page";
import { CronJobsPage } from "../features/cron-jobs/cron-jobs-page";
import { SettingsPage } from "../features/settings/settings-page";
import { CodingPage } from "../features/workspace/coding-page";
import { ChatAgentProvider } from "../features/agents/chat-agent-context";
import { DialogProvider } from "../shared/ui/dialog/dialog-provider";
import { I18nProvider } from "../features/i18n/i18n-context";

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
              <Route path="settings/:sectionId" element={<SettingsPage />} />
              <Route path="gateways" element={<GatewaysPage />} />
              <Route path="cron-jobs" element={<CronJobsPage />} />
              <Route path="*" element={<Navigate to="/" replace />} />
            </Route>
          </Routes>
        </ChatAgentProvider>
      </DialogProvider>
    </I18nProvider>
  );
}
