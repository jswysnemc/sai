import { useQuery } from "@tanstack/react-query";
import { api } from "../../../../api/client";
import type { SandboxConfig, SandboxNetworkMode } from "../../../../api/contracts";
import { useI18n } from "../../../i18n/use-i18n";
import { ChoicePills, FieldGrid, SettingsField, SettingsPanel, SkListInput, SwitchField } from "../../kit";
import type { RuntimeSettingsProps } from "../runtime-settings-types";
import { SandboxStatusSummary } from "./sandbox-status-summary";

/**
 * 【Web 设置】【沙箱】展示后端探测结果并编辑沙箱开关、网络、环境变量与路径。
 * @param props 应用配置与更新回调
 * @returns 沙箱设置面板
 */
export function SandboxSettings({ config, onConfigChange }: RuntimeSettingsProps) {
  const { t } = useI18n();
  const status = useQuery({ queryKey: ["sandbox-status"], queryFn: api.config.sandboxStatus, staleTime: 30_000 });
  const sandbox = config.sandbox ?? {};
  const enabled = sandbox.enabled ?? true;
  const network: SandboxNetworkMode = sandbox.network ?? "deny";
  const scrubEnv = sandbox.scrub_env ?? true;

  /**
   * 合并沙箱补丁，保留未展示字段。
   * @param patch 修改的沙箱字段
   * @returns 无返回值
   */
  const update = (patch: Partial<SandboxConfig>) => onConfigChange({ ...config, sandbox: { ...sandbox, ...patch } });

  const networkOptions = [
    { value: "deny" as const, label: t("Blocked", "断开"), description: t("Commands that need the network ask for an escalation", "需要联网的命令走提升审批") },
    { value: "allow" as const, label: t("Allowed", "允许"), description: t("Only the filesystem is restricted", "只限制文件系统") }
  ];

  return (
    <SettingsPanel
      title={t("Command sandbox", "命令沙箱")}
      description={t("Isolates run_command in audited, auto-audit and plan modes: bubblewrap on Linux, Seatbelt on macOS. Changes apply to the next command.", "隔离审核、自动审核与计划模式下的 run_command：Linux 使用 bubblewrap，macOS 使用 Seatbelt。修改从下一条命令起生效。")}
    >
      <SandboxStatusSummary status={status.data} loading={status.isLoading} failed={status.isError} onRetry={() => void status.refetch()} />
      <FieldGrid>
        <SwitchField
          label={t("Enable sandbox", "启用沙箱")}
          hint={t("When off, audited commands still need approval but run without isolation.", "关闭后审核模式仍逐条审批，但命令不再隔离。")}
          checked={enabled}
          onChange={(value) => update({ enabled: value })}
          configKey="sandbox.enabled"
          anchor="runtime.sandbox.enabled"
        />
        <SwitchField
          label={t("Scrub secret env vars", "清理密钥环境变量")}
          hint={t("Removes API keys, tokens and passwords from the sandboxed command environment.", "从沙箱命令环境中移除 API Key、Token 与密码类变量。")}
          checked={scrubEnv}
          onChange={(value) => update({ scrub_env: value })}
          configKey="sandbox.scrub_env"
          anchor="runtime.sandbox.scrub_env"
        />
        <SettingsField label={t("Network", "网络")} configKey="sandbox.network" anchor="runtime.sandbox.network" span="full">
          <ChoicePills value={network} options={networkOptions} onChange={(value) => update({ network: value })} ariaLabel={t("Sandbox network", "沙箱网络")} />
        </SettingsField>
        <SettingsField label={t("Extra writable roots", "额外可写目录")} hint={t("One path per line, e.g. build caches. ~/ expands to the home directory.", "每行一个路径，例如构建缓存；~/ 表示家目录。")} configKey="sandbox.writable_roots" anchor="runtime.sandbox.writable_roots">
          <SkListInput value={sandbox.writable_roots ?? []} onChange={(value) => update({ writable_roots: value })} placeholder="~/.cargo/registry" />
        </SettingsField>
        <SettingsField label={t("Extra hidden paths", "额外隐藏路径")} hint={t("Added to the built-in credential list (~/.ssh, ~/.aws, sai secrets). Relative paths resolve against the workspace.", "追加到内置凭据清单（~/.ssh、~/.aws、sai 密钥）之后；相对路径按工作区解析。")} configKey="sandbox.deny_read" anchor="runtime.sandbox.deny_read">
          <SkListInput value={sandbox.deny_read ?? []} onChange={(value) => update({ deny_read: value })} placeholder=".env" />
        </SettingsField>
        <SettingsField label={t("Env vars kept when scrubbing", "清理时保留的变量")} hint={t("One variable name per line.", "每行一个变量名。")} configKey="sandbox.env_passthrough" anchor="runtime.sandbox.env_passthrough">
          <SkListInput value={sandbox.env_passthrough ?? []} onChange={(value) => update({ env_passthrough: value })} placeholder="GITHUB_TOKEN" />
        </SettingsField>
      </FieldGrid>
    </SettingsPanel>
  );
}
