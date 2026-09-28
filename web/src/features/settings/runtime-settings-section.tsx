import { RuntimeEnvironmentSettings } from "./runtime/runtime-environment-settings";
import { RuntimeExecutionSettings } from "./runtime/runtime-execution-settings";
import { RuntimeToolsSettings } from "./runtime/runtime-tools-settings";
import type { RuntimeSettingsProps } from "./runtime/runtime-settings-types";

/**
 * 【Web 设置】【运行时路由】组合三类运行时子页，并兼容旧子页标识。
 * @param props 应用配置、当前子页与更新回调
 * @returns 对应领域的设置内容
 */
export function RuntimeSettingsSection({ subview, ...props }: RuntimeSettingsProps & { subview?: string }) {
  switch (subview) {
    case "environment":
    case "permissions":
    case "terminal":
      return <RuntimeEnvironmentSettings {...props} />;
    case "tools":
    case "context":
      return <RuntimeToolsSettings {...props} />;
    default:
      return <RuntimeExecutionSettings {...props} />;
  }
}
