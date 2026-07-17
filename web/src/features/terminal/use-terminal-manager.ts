import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { api } from "../../api/client";
import type { TerminalInfo } from "../../api/contracts";

/**
 * 管理终端列表、当前选择和显式关闭操作。
 *
 * @returns 终端管理状态与操作方法
 */
export function useTerminalManager() {
  const queryClient = useQueryClient();
  const terminals = useQuery({ queryKey: ["terminals"], queryFn: api.terminals.list });
  const [activeId, setActiveId] = useState<string | null>(null);

  useEffect(() => {
    const items = terminals.data?.terminals ?? [];
    if (!activeId) {
      setActiveId(items[0]?.id ?? null);
      return;
    }
    if (!terminals.isFetching && !items.some((item) => item.id === activeId)) setActiveId(items[0]?.id ?? null);
  }, [terminals.data, terminals.isFetching, activeId]);

  /**
   * 创建并选中新终端。
   *
   * @returns 新建终端信息
   */
  const createTerminal = async () => {
    const terminal = await api.terminals.create(100, 28);
    queryClient.setQueryData<{ terminals: TerminalInfo[] }>(["terminals"], (current) => ({
      terminals: [...(current?.terminals ?? []), terminal]
    }));
    setActiveId(terminal.id);
    await queryClient.invalidateQueries({ queryKey: ["terminals"] });
    return terminal;
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
    setActiveId,
    createTerminal,
    closeTerminal,
    renameTerminal
  };
}

export type TerminalManager = ReturnType<typeof useTerminalManager>;
