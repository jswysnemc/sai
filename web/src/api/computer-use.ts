import { apiRequest } from "./api-request";
import type { ComputerUseDriverStatus, ComputerUseInstallResult } from "./contracts/computer-use";

/**
 * 【桌面操作】【状态查询】读取本机 cua-driver 探测结果，不启动 MCP。
 *
 * @returns 驱动探测结果
 */
export function fetchComputerUseStatus(): Promise<ComputerUseDriverStatus> {
  return apiRequest<ComputerUseDriverStatus>("/api/computer-use/status");
}

/**
 * 【桌面操作】【驱动安装】请求服务端下载官方发布包。
 *
 * @returns 安装后的路径与版本
 */
export function installComputerUseDriver(): Promise<ComputerUseInstallResult> {
  return apiRequest<ComputerUseInstallResult>("/api/computer-use/install", {
    method: "POST",
    body: "{}"
  });
}
