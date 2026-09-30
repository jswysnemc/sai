/** 沙箱内网络策略。 */
export type SandboxNetworkMode = "deny" | "allow";

/** 审核与计划模式下 Shell 命令的操作系统沙箱配置。 */
export type SandboxConfig = {
  /** 是否启用沙箱，缺省为开启 */
  enabled?: boolean;
  network?: SandboxNetworkMode;
  /** 工作区之外额外允许写入的目录 */
  writable_roots?: string[];
  /** 在内置凭据目录之外额外隐藏的路径 */
  deny_read?: string[];
  /** 是否移除密钥类环境变量，缺省为开启 */
  scrub_env?: boolean;
  /** 清理时仍保留的变量名 */
  env_passthrough?: string[];
};

/** 服务端探测到的沙箱状态。 */
export type SandboxStatusResponse = {
  enabled: boolean;
  platform_supported: boolean;
  /** bwrap、seatbelt 或 none */
  backend: string;
  available: boolean;
  reason?: string;
  network: SandboxNetworkMode;
  scrub_env: boolean;
  writable_roots: string[];
  deny_read: string[];
  write_protected: string[];
  scrubbed_env: string[];
  temp_dir?: string;
};

/** 命令获批后的执行环境。 */
export type SandboxScopeKind = "sandboxed" | "escalated" | "unsandboxed" | "unavailable";

/** 权限请求附带的沙箱范围。 */
export type SandboxScope = {
  kind: SandboxScopeKind;
  /** bwrap、seatbelt 或 none */
  backend: string;
  /** 沙箱内是否有网络 */
  network: boolean;
  /** 需要提升的原因：requested、network、package_manager、outside_path */
  reasons?: string[];
  /** 模型填写的提升理由 */
  justification?: string;
};

/** 命令结果中的沙箱拦截说明。 */
export type SandboxDenial = {
  kind: "filesystem" | "network";
  evidence?: string;
  hint?: string;
};
