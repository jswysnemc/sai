import { ShieldAlert, ShieldQuestion } from "lucide-react";
import type { SshHostKeyPrompt } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { Modal } from "../../../shared/ui/dialog/modal";
import { useI18n } from "../../i18n/use-i18n";
import "./ssh-known-host-prompt.css";

type SshKnownHostPromptProps = {
  prompt: SshHostKeyPrompt | null;
  busy: boolean;
  onTrust: () => void;
  onCancel: () => void;
};

/**
 * 在首次连接或主机密钥变更时请用户核对指纹。
 *
 * 首次连接允许确认后记入 known_hosts；指纹与已存记录不符时只提示不放行，
 * 那种情况可能是中间人攻击，需用户自行核实后手工处理 known_hosts。
 *
 * @param props 待确认密钥、忙碌状态与确认取消回调
 * @returns 主机密钥确认弹层
 */
export function SshKnownHostPrompt(props: SshKnownHostPromptProps) {
  const { t } = useI18n();
  const prompt = props.prompt;
  if (!prompt) return null;

  const changed = prompt.changed;
  const address = prompt.port === 22 ? prompt.hostname : `${prompt.hostname}:${prompt.port}`;

  return (
    <Modal
      open
      title={changed ? t("Host key changed", "主机密钥已变更") : t("Unknown host key", "未知的主机密钥")}
      description={
        changed
          ? t(
              "The stored key for this host does not match the key it just presented. This may indicate a man-in-the-middle attack.",
              "该主机已登记的密钥与本次返回的密钥不一致，可能存在中间人攻击。"
            )
          : t(
              "This host has not been connected before. Verify the fingerprint through a trusted channel before continuing.",
              "此前没有连接过该主机。请通过可信渠道核对指纹后再继续。"
            )
      }
      size="small"
      onClose={props.onCancel}
      footer={
        <>
          <Button variant="secondary" onClick={props.onCancel} disabled={props.busy}>
            {changed ? t("Close", "关闭") : t("Cancel", "取消")}
          </Button>
          {!changed && (
            <Button variant="primary" onClick={props.onTrust} disabled={props.busy}>
              {t("Trust and connect", "信任并连接")}
            </Button>
          )}
        </>
      }
    >
      <div className={`ssh-known-host ${changed ? "changed" : "unknown"}`}>
        <div className="ssh-known-host-icon">
          {changed ? <ShieldAlert size={18} /> : <ShieldQuestion size={18} />}
        </div>
        <dl>
          <div>
            <dt>{t("Host", "主机")}</dt>
            <dd>{address}</dd>
          </div>
          <div>
            <dt>{t("Algorithm", "算法")}</dt>
            <dd>{prompt.algorithm}</dd>
          </div>
          <div>
            <dt>{t("Fingerprint", "指纹")}</dt>
            <dd className="ssh-known-host-fingerprint">{prompt.fingerprint}</dd>
          </div>
        </dl>
      </div>
      {changed && (
        <p className="ssh-known-host-remedy">
          {t(
            "Remove the stale entry from ~/.ssh/known_hosts once you have confirmed the change is legitimate.",
            "确认变更确属正常后，请手动从 ~/.ssh/known_hosts 中删除旧记录。"
          )}
        </p>
      )}
    </Modal>
  );
}
