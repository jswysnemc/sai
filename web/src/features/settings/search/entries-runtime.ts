import { fieldLabel } from "../structured-field-metadata";
import type { SettingsSearchEntry } from "./settings-search-types";

/** 固定运行时字段：配置路径、中英文名称与所属子页。 */
const RUNTIME_FIELDS = [
  ["agent.engine", "Conversation engine", "对话内核", "execution"],
  ["agent.acp.auth_method", "ACP authentication method", "ACP 认证方式", "execution"],
  ["agent.acp.additional_directories", "Additional directories", "附加目录", "execution"],
  ["agent.acp.command", "Launch command", "启动命令", "execution"],
  ["agent.acp.args", "Startup arguments", "启动参数", "execution"],
  ["session.new_session_model", "New session model", "新会话模型", "execution"],
  ["session.new_session_thinking_level", "New session reasoning effort", "新会话思考等级", "execution"],
  ["session.auto_title_enabled", "Auto title on first turn", "首轮自动标题", "execution"],
  ["session.auto_title_model", "Title model", "标题模型", "execution"],
  ["retry.max_attempts", "Max attempts", "最大尝试次数", "execution"],
  ["retry.initial_delay_ms", "Initial delay", "首次重试间隔", "execution"],
  ["retry.backoff", "Delay schedule", "间隔方式", "execution"],
  ["permission.tui_mode", "TUI default mode", "TUI 默认模式", "environment"],
  ["permission.cli_mode", "CLI default mode", "CLI 默认模式", "environment"],
  ["permission.auto_audit_model", "Auto audit model", "自动审核模型", "environment"],
  ["sandbox.enabled", "Enable sandbox", "启用沙箱", "environment"],
  ["sandbox.scrub_env", "Scrub secret env vars", "清理密钥环境变量", "environment"],
  ["sandbox.network", "Sandbox network", "沙箱网络", "environment"],
  ["sandbox.writable_roots", "Extra writable roots", "额外可写目录", "environment"],
  ["sandbox.deny_read", "Extra hidden paths", "额外隐藏路径", "environment"],
  ["sandbox.env_passthrough", "Env vars kept when scrubbing", "清理时保留的变量", "environment"],
  ["terminal.shell", "Terminal Shell", "终端 Shell", "environment"],
  ["terminal.font", "Terminal font", "终端字体", "environment"],
  ["terminal.font_size", "Terminal font size", "终端字号", "environment"],
  ["terminal.scrollback", "Terminal scrollback lines", "终端回滚行数", "environment"],
  ["input.paste_image_key", "TUI clipboard paste key", "TUI 剪贴板粘贴键", "environment"],
  ["context.default_max_chars", "Default context tokens", "默认上下文 token 数", "tools"],
  ["context.compaction_ratio", "Auto-compact ratio", "自动压缩比例", "tools"],
  ["context.compaction_reserve_tokens", "Reserved headroom", "压缩预留 token", "tools"],
  ["context.compaction_model", "Compaction model", "压缩模型", "tools"],
  ["context.experimental_context_blocks", "Experimental tool-result compression", "实验性工具结果压缩", "tools"],
  ["memory.extraction_model", "Session memory extraction model", "会话记忆点提取模型", "tools"],
  ["tools.command_filter", "Command output filter mode", "命令输出过滤器档位", "tools"],
  ["tools.command_filter_denylist", "Exclude a command", "排除命令", "tools"],
  ["mesh.cross_session", "Cross-session messaging", "跨会话投递", "tools"],
  ["debug.enabled", "Enable API debug", "开启 API 调试", "tools"],
  ["debug.retain_logs", "Retain complete debug logs", "保留完整调试日志", "tools"]
] as const;

/** 当前服务端公开的工具与输出字段，与结构化控件使用相同标签。 */
const STRUCTURED_FIELDS = {
  tools: ["enabled", "background_commands_enabled", "background_command_timeout_seconds", "background_command_log_max_bytes", "background_command_stop_grace_seconds"],
  display: ["reasoning", "tool_calls", "readable_tool_names", "wait_show_model", "wait_show_thinking_level", "repl_transcript_row_cap"]
};

/** 运行时分区的字段搜索索引。 */
export const RUNTIME_SEARCH_ENTRIES: SettingsSearchEntry[] = [
  ...RUNTIME_FIELDS.map(([key, labelEn, labelZh, subview]): SettingsSearchEntry => ({
    anchor: `runtime.${key}`, section: "runtime", subview, labelEn, labelZh,
    keywords: [key, ...(key.startsWith("tools.command_filter") ? ["rtk"] : []), ...(key === "memory.extraction_model" ? ["plugins.memory.extraction_model"] : [])]
  })),
  ...Object.entries(STRUCTURED_FIELDS).flatMap(([group, keys]) => keys.map((key): SettingsSearchEntry => ({
    anchor: `runtime.${group}.${key}`, section: "runtime", subview: "tools",
    labelEn: fieldLabel(key, (en) => en), labelZh: fieldLabel(key, (_en, zh) => zh), keywords: [`${group}.${key}`]
  })))
];
