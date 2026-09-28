import type { SettingsSearchEntry } from "./settings-search-types";

/** 供应商、生图端点与 Jev 字段索引。 */
export const MODEL_SERVICE_SEARCH_ENTRIES: SettingsSearchEntry[] = [
  { anchor: "providers.connection.api_keys", section: "providers", subview: "connection", labelEn: "API keys", labelZh: "接口密钥", keywords: ["providers.api_keys", "api_key"] },
  { anchor: "providers.advanced.user_agent", section: "providers", subview: "advanced", labelEn: "User-Agent", labelZh: "User-Agent", keywords: ["providers.user_agent"] },
  { anchor: "image-models.api_keys", section: "image-models", labelEn: "Image API keys", labelZh: "生图接口密钥", keywords: ["api_key", "credentials"] },
  { anchor: "jev.api_keys", section: "jev", labelEn: "Jev API keys", labelZh: "Jev 接口密钥", keywords: ["api_key", "TYPESAFE_API_KEY"] },

  {
    "anchor": "providers.connection.base_url",
    "section": "providers",
    "subview": "connection",
    "labelEn": "API address",
    "labelZh": "API 地址",
    "keywords": [
      "providers.connection.base_url"
    ]
  },
  {
    "anchor": "providers.connection.default_model",
    "section": "providers",
    "subview": "connection",
    "labelEn": "Default model",
    "labelZh": "默认模型",
    "keywords": [
      "providers.connection.default_model"
    ]
  },
  {
    "anchor": "providers.connection.protocol",
    "section": "providers",
    "subview": "connection",
    "labelEn": "Protocol",
    "labelZh": "协议",
    "keywords": [
      "providers.connection.protocol"
    ]
  },
  {
    "anchor": "providers.connection.display_name",
    "section": "providers",
    "subview": "connection",
    "labelEn": "Display name",
    "labelZh": "显示名称",
    "keywords": [
      "providers.connection.display_name"
    ]
  },
  {
    "anchor": "providers.connection.id",
    "section": "providers",
    "subview": "connection",
    "labelEn": "Provider ID",
    "labelZh": "供应商 ID",
    "keywords": [
      "providers.connection.id"
    ]
  },
  {
    "anchor": "providers.behavior.timeout_seconds",
    "section": "providers",
    "subview": "behavior",
    "labelEn": "Request timeout",
    "labelZh": "请求超时",
    "keywords": [
      "providers.behavior.timeout_seconds"
    ]
  },
  {
    "anchor": "providers.behavior.temperature",
    "section": "providers",
    "subview": "behavior",
    "labelEn": "Temperature",
    "labelZh": "温度",
    "keywords": [
      "providers.behavior.temperature"
    ]
  },
  {
    "anchor": "providers.behavior.thinking_level",
    "section": "providers",
    "subview": "behavior",
    "labelEn": "Thinking level",
    "labelZh": "思考等级",
    "keywords": [
      "providers.behavior.thinking_level"
    ]
  },
  {
    "anchor": "providers.behavior.thinking_format",
    "section": "providers",
    "subview": "behavior",
    "labelEn": "Thinking format",
    "labelZh": "思考格式",
    "keywords": [
      "providers.behavior.thinking_format"
    ]
  },
  {
    "anchor": "providers.behavior.preserve_thinking",
    "section": "providers",
    "subview": "behavior",
    "labelEn": "Preserve thinking",
    "labelZh": "回传历史思考",
    "keywords": [
      "providers.behavior.preserve_thinking"
    ]
  },
  {
    "anchor": "providers.behavior.deepseek_anchor",
    "section": "providers",
    "subview": "behavior",
    "labelEn": "DeepSeek trajectory anchor",
    "labelZh": "DeepSeek 轨迹锚定",
    "keywords": [
      "providers.behavior.deepseek_anchor"
    ]
  },
  {
    "anchor": "providers.behavior.anthropic_max_tokens",
    "section": "providers",
    "subview": "behavior",
    "labelEn": "Claude max output",
    "labelZh": "Claude 最大输出",
    "keywords": [
      "providers.behavior.anthropic_max_tokens"
    ]
  },
  {
    "anchor": "providers.advanced.client_style",
    "section": "providers",
    "subview": "advanced",
    "labelEn": "Client style",
    "labelZh": "客户端模拟",
    "keywords": [
      "providers.advanced.client_style"
    ]
  },
  {
    "anchor": "providers.advanced.claude_1m_context",
    "section": "providers",
    "subview": "advanced",
    "labelEn": "Claude 1M context",
    "labelZh": "Claude 启用 1M 上下文",
    "keywords": [
      "providers.advanced.claude_1m_context"
    ]
  },
  {
    "anchor": "providers.advanced.extra_headers",
    "section": "providers",
    "subview": "advanced",
    "labelEn": "Extra headers",
    "labelZh": "自定义请求头",
    "keywords": [
      "providers.advanced.extra_headers"
    ]
  },
  {
    "anchor": "providers.advanced.extra_body",
    "section": "providers",
    "subview": "advanced",
    "labelEn": "Custom body JSON",
    "labelZh": "自定义 body JSON",
    "keywords": [
      "providers.advanced.extra_body"
    ]
  },
  {
    "anchor": "providers.models.context_chars",
    "section": "providers",
    "subview": "models",
    "labelEn": "Context tokens",
    "labelZh": "上下文 token 数",
    "keywords": [
      "providers.models.context_chars"
    ]
  },
  {
    "anchor": "providers.models.max_output_tokens",
    "section": "providers",
    "subview": "models",
    "labelEn": "Maximum output tokens",
    "labelZh": "最大输出 token 数",
    "keywords": [
      "providers.models.max_output_tokens"
    ]
  },
  {
    "anchor": "providers.models.tools_enabled",
    "section": "providers",
    "subview": "models",
    "labelEn": "Tool calls",
    "labelZh": "工具调用",
    "keywords": [
      "providers.models.tools_enabled"
    ]
  },
  {
    "anchor": "providers.models.web_search_tool_mode",
    "section": "providers",
    "subview": "models",
    "labelEn": "Web search tool",
    "labelZh": "网页搜索工具",
    "keywords": [
      "providers.models.web_search_tool_mode"
    ]
  },
  {
    "anchor": "providers.models.thinking_levels",
    "section": "providers",
    "subview": "models",
    "labelEn": "Supported reasoning levels",
    "labelZh": "支持的推理强度",
    "keywords": [
      "providers.models.thinking_levels"
    ]
  },
  {
    "anchor": "providers.models.tags",
    "section": "providers",
    "subview": "models",
    "labelEn": "Model tags",
    "labelZh": "模型标签",
    "keywords": [
      "providers.models.tags"
    ]
  },
  {
    "anchor": "image-models.endpoint",
    "section": "image-models",
    "labelEn": "API address",
    "labelZh": "API 地址",
    "keywords": [
      "image-models.endpoint"
    ]
  },
  {
    "anchor": "image-models.model",
    "section": "image-models",
    "labelEn": "Model",
    "labelZh": "模型",
    "keywords": [
      "image-models.model"
    ]
  },
  {
    "anchor": "image-models.protocol",
    "section": "image-models",
    "labelEn": "Protocol",
    "labelZh": "协议",
    "keywords": [
      "image-models.protocol"
    ]
  },
  {
    "anchor": "image-models.name",
    "section": "image-models",
    "labelEn": "Display name",
    "labelZh": "显示名称",
    "keywords": [
      "image-models.name"
    ]
  },
  {
    "anchor": "jev.endpoint",
    "section": "jev",
    "labelEn": "API address",
    "labelZh": "API 地址",
    "keywords": [
      "jev.endpoint"
    ]
  },
  {
    "anchor": "jev.model",
    "section": "jev",
    "labelEn": "Model",
    "labelZh": "模型",
    "keywords": [
      "jev.model"
    ]
  },
  {
    "anchor": "jev.name",
    "section": "jev",
    "labelEn": "Display name",
    "labelZh": "显示名称",
    "keywords": [
      "jev.name"
    ]
  },
  {
    "anchor": "jev.endpoint_id",
    "section": "jev",
    "labelEn": "Jev connection",
    "labelZh": "Jev 接入",
    "keywords": [
      "jev.endpoint_id"
    ]
  },
  {
    "anchor": "jev.routing.enabled",
    "section": "jev",
    "labelEn": "Enable routing",
    "labelZh": "启用暴露决策",
    "keywords": [
      "jev.routing.enabled"
    ]
  },
  {
    "anchor": "jev.routing.threshold",
    "section": "jev",
    "labelEn": "Minimum probability",
    "labelZh": "最低概率",
    "keywords": [
      "jev.routing.threshold"
    ]
  },
  {
    "anchor": "jev.routing.max_tools",
    "section": "jev",
    "labelEn": "Max tools per decision",
    "labelZh": "单次最多工具数",
    "keywords": [
      "jev.routing.max_tools"
    ]
  },
  {
    "anchor": "jev.routing.max_skills",
    "section": "jev",
    "labelEn": "Max skills per decision",
    "labelZh": "单次最多 Skills 数",
    "keywords": [
      "jev.routing.max_skills"
    ]
  },
  {
    "anchor": "jev.routing.timeout_seconds",
    "section": "jev",
    "labelEn": "Timeout (seconds)",
    "labelZh": "超时（秒）",
    "keywords": [
      "jev.routing.timeout_seconds"
    ]
  },
  {
    "anchor": "jev.routing.context_chars",
    "section": "jev",
    "labelEn": "Context characters",
    "labelZh": "判断所用对话字符数",
    "keywords": [
      "jev.routing.context_chars"
    ]
  },
  {
    "anchor": "jev.audit.enabled",
    "section": "jev",
    "labelEn": "Enable Jev audit",
    "labelZh": "启用 Jev 审核",
    "keywords": [
      "jev.audit.enabled"
    ]
  },
  {
    "anchor": "jev.audit.minimum_probability",
    "section": "jev",
    "labelEn": "Minimum choice probability",
    "labelZh": "最低选项概率",
    "keywords": [
      "jev.audit.minimum_probability"
    ]
  },
  {
    "anchor": "jev.audit.minimum_confidence",
    "section": "jev",
    "labelEn": "Minimum confidence",
    "labelZh": "最低置信度",
    "keywords": [
      "jev.audit.minimum_confidence"
    ]
  },
  {
    "anchor": "jev.audit.timeout_seconds",
    "section": "jev",
    "labelEn": "Timeout (seconds)",
    "labelZh": "超时（秒）",
    "keywords": [
      "jev.audit.timeout_seconds"
    ]
  }
];
