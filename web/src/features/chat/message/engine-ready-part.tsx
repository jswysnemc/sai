import { AgentEngineBrandIcon } from "../../../shared/ui/agent-engine-brand-icon/agent-engine-brand-icon";
import { useI18n } from "../../i18n/use-i18n";
import "./engine-ready-part.css";

type EngineReadyPartProps = {
  /** 外部内核名称，来自 ACP 握手响应的 agentInfo */
  engine: string;
  /** 外部内核版本 */
  version: string;
};

/**
 * 渲染外部内核已连接的提示。
 *
 * 名称与版本取自 ACP 握手响应，只有真正拉起外部进程并完成握手才拿得到，
 * 因此这一行是「本轮由谁执行」的运行时证据——不是配置读数的复述。
 * 看不到这一行就说明仍在用 sai 自带内核。
 *
 * @param props 内核名称与版本
 * @returns 连接提示行
 */
export function EngineReadyPart({ engine, version }: EngineReadyPartProps) {
  const { t } = useI18n();
  return (
    <div className="engine-ready-part" role="note">
      <AgentEngineBrandIcon engine={engine} size={12} />
      <span>{t("Handed off to", "已交由")}</span>
      <strong>{engine}</strong>
      {version && <code>{version}</code>}
    </div>
  );
}
