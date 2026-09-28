import { lazy, Suspense } from "react";
import { Button } from "../../../shared/ui/button/button";
import { Modal } from "../../../shared/ui/dialog/modal";
import { useI18n } from "../../i18n/use-i18n";
import { InlineNotice } from "../kit";
import { sshHostAddress } from "./ssh-host-form-state";
import { SshKnownHostPrompt } from "./ssh-known-host-prompt";
import type { SshTerminalController } from "./use-ssh-terminal";

const TerminalPane = lazy(() => import("../../terminal/terminal-pane").then((module) => ({ default: module.TerminalPane })));

/**
 * 【SSH 设置】【终端窗口】展示实际终端或连接状态，首次连接先确认主机指纹。
 * @param props 连接控制器
 * @returns 终端窗口与主机指纹确认
 */
export function SshTerminalDialog({ connection }: { connection: SshTerminalController }) {
  const { t } = useI18n();
  const { host, terminal, busy, error, hostKeyPrompt } = connection;
  return <>
    <Modal open={Boolean(host) && !hostKeyPrompt} title={host ? `${host.label} · SSH` : "SSH"}
      description={host ? sshHostAddress(host) : undefined} size="large" onClose={connection.hide}
      footer={<>
        <Button disabled={busy} onClick={connection.hide}>{t("Hide", "收起")}</Button>
        {terminal ? <Button variant="danger" disabled={busy} onClick={() => void connection.end()}>{t("End terminal", "结束终端")}</Button>
          : error && host && <Button variant="primary" disabled={busy} onClick={() => void connection.open(host)}>{t("Retry", "重试")}</Button>}
      </>}>
      {error && <InlineNotice tone="danger">{error}</InlineNotice>}
      {terminal ? <>
        <div className="h-[min(60dvh,32rem)] min-h-48 overflow-hidden">
          <Suspense fallback={<InlineNotice>{t("Loading terminal", "正在加载终端")}</InlineNotice>}>
            <TerminalPane terminalId={terminal.id} title={host?.label ?? terminal.title} />
          </Suspense>
        </div>
        <p className="mt-2 text-xs text-muted">{t("Hiding this window keeps the session available in the workspace terminal panel.", "收起窗口后，会话仍可在主界面的终端面板中继续使用。")}</p>
      </> : busy && <InlineNotice>{t("Connecting to host", "正在连接主机")}</InlineNotice>}
    </Modal>
    <SshKnownHostPrompt prompt={hostKeyPrompt} error={error} busy={busy} onTrust={() => void connection.trust()} onCancel={connection.hide} />
  </>;
}
