import { useMutation } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { useSelectedFallback } from "../controls/use-selected-fallback";
import { api } from "../../../api/client";
import type { McpConfig, McpServerConfig } from "../../../api/contracts";
import { useConfigDocument } from "../use-config-document";
import { createDefaultMcpServer, parseMcpJson, uniqueServerId } from "./mcp-helpers";

export type McpEditorMode = "form" | "json";

/**
 * 管理独立 MCP 配置的加载、草稿、脏标记与保存。
 *
 * 文档状态机复用 useConfigDocument；本 Hook 补充表单/JSON 双模式、
 * 服务列表编辑与工具扫描等 MCP 专属逻辑。
 *
 * @returns MCP 配置控制器
 */
export function useMcpConfig() {
  const document = useConfigDocument({
    queryKey: ["mcp-config"] as const,
    load: api.config.loadMcp,
    extract: (response) => response.config,
    save: (config: McpConfig) => api.config.saveMcp(config)
  });
  const mcp = document.draft;
  const [raw, setRaw] = useState("");
  const [mode, setMode] = useState<McpEditorMode>("form");
  const [selectedId, setSelectedId] = useState("");
  const [parseError, setParseError] = useState<string | null>(null);
  const [scannedServerId, setScannedServerId] = useState("");

  useEffect(() => {
    // 1. 无本地编辑时 JSON 文本跟随草稿（首次加载与保存回填）
    if (document.dirty || !mcp) return;
    setRaw(JSON.stringify(mcp, null, 2));
    setParseError(null);
  }, [mcp, document.dirty]);

  const servers = mcp?.servers ?? [];
  useSelectedFallback(selectedId, servers.map((server) => server.id), setSelectedId);

  const selectedIndex = Math.max(0, servers.findIndex((server) => server.id === selectedId));
  const server = servers[selectedIndex];

  /** 保存外观：JSON 模式提交解析结果，成功后同步文本。 */
  const save = {
    error: document.saveError,
    isPending: document.saving,
    mutateAsync: async () => {
      const saved = await document.saveNow(mode === "json" ? parseMcpJson(raw) : undefined);
      setRaw(JSON.stringify(saved.config, null, 2));
      setParseError(null);
    }
  };

  const scanTools = useMutation({
    mutationFn: (target: McpServerConfig) => api.config.scanMcpTools(target),
    onSuccess: (_, target) => setScannedServerId(target.id)
  });

  /**
   * 用完整配置替换草稿并同步 JSON。
   *
   * @param next 新 MCP 配置
   */
  const updateMcp = (next: McpConfig) => {
    document.update(next);
    setRaw(JSON.stringify(next, null, 2));
    setParseError(null);
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
      document.update(parsed);
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
    // 1. JSON 合法时同步到表单状态，便于切回表单不丢内容；
    //    非法输入只标记待保存，保存时再报解析错误
    try {
      document.update(parseMcpJson(value));
    } catch {
      document.markDirty();
    }
  };

  return {
    loading: document.loading,
    path: document.response.data?.path ?? "~/.config/sai/mcp.jsonc",
    loadError: document.loadError,
    mcp,
    raw,
    dirty: document.dirty,
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
