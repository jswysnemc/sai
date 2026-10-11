/** 原生桌面操作配置。 */
export type ComputerUseConfig = {
  /** 用户是否明确启用原生桌面操作 */
  enabled: boolean;
  /** 本地 cua-driver 可执行文件，可使用绝对路径 */
  driver_path: string;
  /** 单次原生调用超时，毫秒 */
  timeout_ms: number;
};

/** 本机 cua-driver 探测结果。 */
export type ComputerUseDriverStatus = {
  platform: string;
  arch: string;
  configured_path: string;
  resolved_path?: string;
  installed: boolean;
  version?: string;
  version_ok: boolean;
  min_version: string;
  pinned_version: string;
  install_supported: boolean;
  install_path: string;
  version_error?: string;
};

/** 驱动安装结果。 */
export type ComputerUseInstallResult = {
  path: string;
  version: string;
  platform: string;
};
