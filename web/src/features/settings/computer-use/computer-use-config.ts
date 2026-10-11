import type { AppConfig } from "../../../api/contracts";
import type { ComputerUseConfig } from "../../../api/contracts/computer-use";

/** 默认桌面操作配置，功能保持关闭。 */
export const DEFAULT_COMPUTER_USE: ComputerUseConfig = {
  enabled: false,
  driver_path: "cua-driver",
  timeout_ms: 30_000
};

const MIN_TIMEOUT_MS = 1_000;
const MAX_TIMEOUT_MS = 120_000;

/**
 * 【桌面操作】【配置读取】取出草稿中的桌面配置，缺省字段回落到默认值。
 *
 * @param config 应用配置
 * @returns 桌面操作配置
 */
export function readComputerUse(config: AppConfig): ComputerUseConfig {
  const raw = config.computer_use;
  return {
    enabled: raw?.enabled ?? DEFAULT_COMPUTER_USE.enabled,
    driver_path: raw?.driver_path?.trim() || DEFAULT_COMPUTER_USE.driver_path,
    timeout_ms: clampTimeout(raw?.timeout_ms ?? DEFAULT_COMPUTER_USE.timeout_ms)
  };
}

/**
 * 【桌面操作】【超时夹紧】把超时限制在 1000 至 120000 毫秒。
 *
 * @param value 输入值
 * @returns 合法超时
 */
export function clampTimeout(value: number): number {
  if (!Number.isFinite(value)) return DEFAULT_COMPUTER_USE.timeout_ms;
  return Math.min(MAX_TIMEOUT_MS, Math.max(MIN_TIMEOUT_MS, Math.round(value)));
}

/**
 * 【桌面操作】【配置写入】合并桌面操作字段并回写应用配置。
 *
 * @param config 当前应用配置
 * @param patch 字段补丁
 * @returns 更新后的应用配置
 */
export function writeComputerUse(config: AppConfig, patch: Partial<ComputerUseConfig>): AppConfig {
  const current = readComputerUse(config);
  const driverPath = (patch.driver_path ?? current.driver_path).trim() || DEFAULT_COMPUTER_USE.driver_path;
  return {
    ...config,
    computer_use: {
      enabled: patch.enabled ?? current.enabled,
      driver_path: driverPath,
      timeout_ms: clampTimeout(patch.timeout_ms ?? current.timeout_ms)
    }
  };
}

/**
 * 【桌面操作】【路径替换】判断安装后是否应把草稿路径改成新二进制。
 *
 * @param current 当前草稿路径
 * @param installPath 本次安装路径
 * @returns 应替换时为真
 */
export function shouldReplaceDriverPath(current: string, installPath: string): boolean {
  const value = current.trim();
  if (!value || value === "cua-driver" || value === "cua-driver.exe") return true;
  return value === installPath.trim();
}
