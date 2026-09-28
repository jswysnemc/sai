import { useRef, useState } from "react";
import type { SshHost, TerminalInfo } from "../../../api/contracts";
import { toDisplayError } from "../../../api/api-error";
import { useTerminalManager } from "../../terminal/use-terminal-manager";

/**
 * 【SSH 设置】【终端连接】复用主机密钥确认与终端缓存，区分隐藏窗口和结束会话。
 * @returns 当前主机、终端、请求状态和用户操作
 */
export function useSshTerminal() {
  const manager = useTerminalManager();
  const [host, setHost] = useState<SshHost | null>(null);
  const [terminal, setTerminal] = useState<TerminalInfo | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const pending = useRef(false);

  /**
   * 执行一次连接操作，重复触发不会创建多个终端。
   * @param action 创建终端或信任指纹后的重试
   * @returns 请求结束后无返回值
   */
  const connect = async (action: () => Promise<TerminalInfo | null>) => {
    if (pending.current) return;
    pending.current = true;
    setBusy(true);
    setError(null);
    try {
      setTerminal(await action());
    } catch (cause) {
      setError(toDisplayError(cause, "SSH connection failed", "SSH 连接失败").message);
    } finally {
      pending.current = false;
      setBusy(false);
    }
  };

  /** 打开指定主机的新终端；参数为已保存主机，返回连接请求。 */
  const open = async (target: SshHost) => {
    if (pending.current) return;
    manager.dismissHostKeyPrompt();
    setHost(target);
    setTerminal(null);
    await connect(() => manager.createSshTerminal(target.id));
  };

  /** 隐藏窗口并取消尚未信任的连接；不结束已创建终端。 */
  const hide = () => {
    if (pending.current) return;
    manager.dismissHostKeyPrompt();
    setHost(null);
    setTerminal(null);
    setError(null);
  };

  /** 用户明确结束当前终端，成功后关闭窗口；返回操作请求。 */
  const end = async () => {
    if (!terminal || pending.current) return;
    pending.current = true;
    setBusy(true);
    setError(null);
    try {
      await manager.closeTerminal(terminal.id);
      setTerminal(null);
      setHost(null);
    } catch (cause) {
      setError(toDisplayError(cause, "Failed to end terminal", "结束终端失败").message);
    } finally {
      pending.current = false;
      setBusy(false);
    }
  };

  return { host, terminal, busy, error, hostKeyPrompt: manager.hostKeyPrompt, open, hide, end, trust: () => connect(manager.trustHostKeyAndRetry) };
}

export type SshTerminalController = ReturnType<typeof useSshTerminal>;
