import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { api } from "../../api/client";
import type { SshHostKeyPrompt, TerminalInfo } from "../../api/contracts";

/**
 * 管理终端列表、当前选择和显式关闭操作。
 *
 * @returns 终端管理状态与操作方法
 */
export function useTerminalManager() {
  const queryClient = useQueryClient();
  const terminals = useQuery({ queryKey: ["terminals"], queryFn: api.terminals.list });
  const [activeId, setActiveId] = useState<string | null>(null);
  const [hostKeyPrompt, setHostKeyPrompt] = useState<SshHostKeyPrompt | null>(null);
  const [pendingSshHostId, setPendingSshHostId] = useState<string | null>(null);

  useEffect(() => {
    const items = terminals.data?.terminals ?? [];
    if (!activeId) {
      setActiveId(items[0]?.id ?? null);
      return;
    }
    if (!terminals.isFetching && !items.some((item) => item.id === activeId)) setActiveId(items[0]?.id ?? null);
  }, [terminals.data, terminals.isFetching, activeId]);

  /**
   * 把新建终端并入列表并选中。
   *
   * @param terminal 新建终端信息
   * @returns 新建终端信息
   */
  const adopt = async (terminal: TerminalInfo) => {
    queryClient.setQueryData<{ terminals: TerminalInfo[] }>(["terminals"], (current) => ({
      terminals: [...(current?.terminals ?? []), terminal]
    }));
    setActiveId(terminal.id);
    await queryClient.invalidateQueries({ queryKey: ["terminals"] });
    return terminal;
  };

  /**
   * 创建并选中新的本地终端。
   *
   * @returns 新建终端信息
   */
  const createTerminal = async () => {
    return adopt(await api.terminals.create(100, 28));
  };

  /**
   * 创建并选中新的 SSH 终端。
   *
   * 主机密钥尚未信任时后端不会建立连接，而是回传指纹；
   * 此时记下待连接主机，等用户确认后再重试。
   *
   * @param sshHostId 目标主机标识
   * @returns 新建终端信息；待确认主机密钥时返回 null
   */
  const createSshTerminal = async (sshHostId: string) => {
    const result = await api.terminals.createSsh(sshHostId, 100, 28);
    if (result.host_key_prompt) {
      setPendingSshHostId(sshHostId);
      setHostKeyPrompt(result.host_key_prompt);
      return null;
    }
    setPendingSshHostId(null);
    return adopt(result);
  };

  /**
   * 信任当前待确认的主机密钥并重试连接。
   *
   * @returns 新建终端信息；无待确认项时返回 null
   */
  const trustHostKeyAndRetry = async () => {
    if (!hostKeyPrompt || !pendingSshHostId) return null;
    await api.ssh.trust(hostKeyPrompt);
    setHostKeyPrompt(null);
    return createSshTerminal(pendingSshHostId);
  };

  /** 放弃本次 SSH 连接。 */
  const dismissHostKeyPrompt = () => {
    setHostKeyPrompt(null);
    setPendingSshHostId(null);
  };

  /** 显式终止并移除终端。 */
  const closeTerminal = async (id: string) => {
    await api.terminals.remove(id);
    if (activeId === id) setActiveId(null);
    await queryClient.invalidateQueries({ queryKey: ["terminals"] });
  };

  /** 更新终端标签标题。 */
  const renameTerminal = async (id: string, title: string) => {
    const terminal = await api.terminals.rename(id, title);
    queryClient.setQueryData<{ terminals: TerminalInfo[] }>(["terminals"], (current) => ({
      terminals: (current?.terminals ?? []).map((item) => item.id === id ? terminal : item)
    }));
  };

  return {
    terminals: terminals.data?.terminals ?? [],
    activeId,
    loading: terminals.isLoading,
    error: terminals.error as Error | null,
    hostKeyPrompt,
    setActiveId,
    createTerminal,
    createSshTerminal,
    trustHostKeyAndRetry,
    dismissHostKeyPrompt,
    closeTerminal,
    renameTerminal
  };
}

export type TerminalManager = ReturnType<typeof useTerminalManager>;
