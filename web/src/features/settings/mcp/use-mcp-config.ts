import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { api } from "../../../api/client";
import type { McpConfig, McpServerConfig } from "../../../api/contracts";
import { createDefaultMcpServer, parseMcpJson, uniqueServerId } from "./mcp-helpers";

export type McpEditorMode = "form" | "json";

/**
 * 管理独立 MCP 配置的加载、草稿、脏标记与保存。
 *
 * @returns MCP 配置控制器
 */
export function useMcpConfig() {
  const queryClient = useQueryClient();
  const response = useQuery({ queryKey: ["mcp-config"], queryFn: api.config.loadMcp });
  const [mcp, setMcp] = useState<McpConfig | null>(null);
  const [raw, setRaw] = useState("");
  const [dirty, setDirty] = useState(false);
  const [mode, setMode] = useState<McpEditorMode>("form");
  const [selectedId, setSelectedId] = useState("");
  const [parseError, setParseError] = useState<string | null>(null);
  const [scannedServerId, setScannedServerId] = useState("");

  useEffect(() => {
    if (!response.data || dirty) return;
    setMcp(response.data.config);
    setRaw(JSON.stringify(response.data.config, null, 2));
    setParseError(null);
  }, [response.data, dirty]);

  const servers = mcp?.servers ?? [];
  useEffect(() => {
    if (!servers.some((server) => server.id === selectedId)) {
      setSelectedId(servers[0]?.id ?? "");
    }
  }, [servers, selectedId]);

  const selectedIndex = Math.max(0, servers.findIndex((server) => server.id === selectedId));
  const server = servers[selectedIndex];

  const save = useMutation({
    mutationFn: async () => {
      const config = mode === "json" ? parseMcpJson(raw) : mcp;
      if (!config) throw new Error("MCP config is not loaded");
      return api.config.saveMcp(config);
    },
    onSuccess: (saved) => {
      setMcp(saved.config);
      setRaw(JSON.stringify(saved.config, null, 2));
      setDirty(false);
      setParseError(null);
      queryClient.setQueryData(["mcp-config"], saved);
    }
  });

  const scanTools = useMutation({
    mutationFn: (target: McpServerConfig) => api.config.scanMcpTools(target),
    onSuccess: (_, target) => setScannedServerId(target.id)
  });

  /** 标记未保存并清空上次保存错误。 */
  const markDirty = () => {
    setDirty(true);
    save.reset();
  };

  /**
   * 用完整配置替换草稿并同步 JSON。
   *
   * @param next 新 MCP 配置
   */
  const updateMcp = (next: McpConfig) => {
    setMcp(next);
    setRaw(JSON.stringify(next, null, 2));
    setParseError(null);
    markDirty();
  };

  /**
   * 合并顶层 MCP 字段。
   *
   * @param patch 字段补丁
   */
  const patchMcp = (patch: Partial<McpConfig>) => {
    if (!mcp) return;
    updateMcp({ ...mcp, ...patch });
  };

  /**
   * 更新指定下标的服务配置。
   *
   * @param index 服务下标
   * @param patch 服务字段补丁
   */
  const updateServer = (index: number, patch: Partial<McpServerConfig>) => {
    if (!mcp) return;
    const nextServers = servers.map((item, i) => (i === index ? { ...item, ...patch } : item));
    updateMcp({ ...mcp, servers: nextServers });
    scanTools.reset();
    setScannedServerId("");
    if (index === selectedIndex && patch.id !== undefined) setSelectedId(patch.id);
  };

  /** 追加默认 stdio 服务并选中。 */
  const addServer = () => {
    if (!mcp) return;
    const id = uniqueServerId(servers);
    updateMcp({ ...mcp, servers: [...servers, createDefaultMcpServer(id)] });
    setSelectedId(id);
  };

  /**
   * 删除指定下标服务。
   *
   * @param index 服务下标
   */
  const removeServerAt = (index: number) => {
    if (!mcp) return;
    const next = servers.filter((_, itemIndex) => itemIndex !== index);
    updateMcp({ ...mcp, servers: next });
    setSelectedId(next[0]?.id ?? "");
  };

  /**
   * 切换表单 / JSON 模式，尽量保留草稿。
   *
   * @param next 目标模式
   */
  const switchMode = (next: McpEditorMode) => {
    if (next === mode) return;
    if (next === "json") {
      if (mcp) setRaw(JSON.stringify(mcp, null, 2));
      setParseError(null);
      setMode("json");
      return;
    }
    try {
      const parsed = parseMcpJson(raw);
      setMcp(parsed);
      setParseError(null);
      setMode("form");
    } catch (error) {
      setParseError(error instanceof Error ? error.message : String(error));
    }
  };

  /**
   * 更新 JSON 草稿；合法时同步表单对象。
   *
   * @param value JSON 文本
   */
  const updateRaw = (value: string) => {
    setRaw(value);
    setParseError(null);
    markDirty();
    // 1. JSON 合法时同步到表单状态，便于切回表单不丢内容
    try {
      setMcp(parseMcpJson(value));
    } catch {
      // 输入中途不合法时保留上一份 mcp
    }
  };

  return {
    loading: response.isLoading,
    path: response.data?.path ?? "~/.config/sai/mcp.jsonc",
    loadError: response.error as Error | null,
    mcp,
    raw,
    dirty,
    mode,
    selectedId,
    selectedIndex,
    server,
    servers,
    parseError,
    setParseError,
    scannedServerId,
    save,
    scanTools,
    setSelectedId,
    patchMcp,
    updateServer,
    addServer,
    removeServerAt,
    switchMode,
    updateRaw
  };
}
