/** Jev 工具与 skills 暴露决策配置。 */
export type JevRoutingConfig = {
  enabled: boolean;
  /** 候选被选中所需的最低 Noul 概率 */
  threshold: number;
  max_tools: number;
  max_skills: number;
  timeout_seconds: number;
  /** 作为判断依据的近期对话最大字符数 */
  context_chars: number;
  /** 是否由 Jev 判断 `<jev>` 标签提示词片段；关闭时标签原文随静态提示发送 */
  prompt_segments: boolean;
};

/** Jev 权限自动审核配置。 */
export type JevAuditConfig = {
  enabled: boolean;
  minimum_probability: number;
  minimum_confidence: number;
  timeout_seconds: number;
};

/** 内置 Jev 功能配置。 */
export type JevConfig = {
  /** 引用 model_endpoints 中的 JEV 接入；空为自动 */
  endpoint_id: string;
  routing: JevRoutingConfig;
  audit: JevAuditConfig;
};

/** 不含密钥的接入信息。 */
export type JevConnectionInfo = {
  source: "endpoint" | "official";
  endpoint_id: string | null;
  name: string;
  endpoint: string;
  model: string;
};

/** 已保存配置下的 Jev 状态。 */
export type JevStatus = {
  connection: JevConnectionInfo | null;
  key_ready: boolean;
  error: string | null;
  routing_enabled: boolean;
  routing_active: boolean;
  audit_enabled: boolean;
  /** 自动审核实际使用的后端 */
  audit_backend: "jev" | "plugin" | "model";
};

/** 连接测试结果。 */
export type JevProbeReport = {
  ok: boolean;
  endpoint: string;
  model: string;
  served_model: string | null;
  duration_ms: number;
  detail: string;
};
