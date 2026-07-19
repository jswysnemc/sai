import { CloudUpload } from "lucide-react";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";

type PublishRepositoryControlProps = {
  remoteUrl: string;
  remoteConfigured: boolean;
  canPublish: boolean;
  busy: boolean;
  onRemoteUrlChange: (value: string) => void;
  onSave: () => void;
  onPublish: () => void;
};

/**
 * 渲染 origin 地址配置和首次发布操作。
 *
 * @param props 远端状态、输入值与保存发布回调
 * @returns 远端仓库控制区
 */
export function PublishRepositoryControl(props: PublishRepositoryControlProps) {
  const { t } = useI18n();
  const action = props.canPublish ? props.onPublish : props.onSave;

  return (
    <div className="git-remote-box">
      <span>
        {props.remoteConfigured
          ? t("Remote origin", "远端 origin")
          : props.canPublish
            ? t("Publish to GitHub or another remote", "发布到 GitHub 或其他远端")
            : t("Set origin remote", "设置 origin 远端")}
      </span>
      <input
        value={props.remoteUrl}
        onChange={(event) => props.onRemoteUrlChange(event.target.value)}
        placeholder="git@github.com:owner/repository.git"
        spellCheck={false}
      />
      <Button disabled={!props.remoteUrl.trim() || props.busy} onClick={action}>
        {props.canPublish && <CloudUpload size={13} />}
        {props.remoteConfigured
          ? t("Update remote", "更新远端")
          : props.canPublish
            ? t("Publish Repository", "发布仓库")
            : t("Save remote", "保存远端")}
      </Button>
    </div>
  );
}
