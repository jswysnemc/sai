import { useI18n } from "../../i18n/use-i18n";
import { TextFieldRow } from "../controls/field-row";
import { validateSshHostForm, type SshHostFormState } from "./ssh-host-form-state";

type SshHostFormProps = {
  form: SshHostFormState;
  onChange: (form: SshHostFormState) => void;
};

/**
 * 渲染 SSH 主机的编辑表单。
 *
 * 只收集连接所需的地址与私钥路径：密码与私钥口令不在此保存，
 * 需要口令的私钥在建立连接时单独询问，避免凭据随配置落盘。
 *
 * @param props 表单编辑态与更新回调
 * @returns SSH 主机表单
 */
export function SshHostForm(props: SshHostFormProps) {
  const { t } = useI18n();
  const errors = validateSshHostForm(props.form);

  /**
   * 更新表单单个字段。
   *
   * @param patch 字段补丁
   * @returns 无
   */
  const update = (patch: Partial<SshHostFormState>) => {
    props.onChange({ ...props.form, ...patch });
  };

  return (
    <div className="ssh-host-form">
      <TextFieldRow
        label={t("Name", "名称")}
        hint={t("Shown in the terminal target list; defaults to the hostname.", "展示在终端目标列表中，留空则使用主机名。")}
        value={props.form.label}
        placeholder={t("Build server", "构建服务器")}
        onChange={(label) => update({ label })}
      />
      <TextFieldRow
        label={t("Host", "主机")}
        hint={errors.hostname ? t("Host is required.", "主机不能为空。") : undefined}
        value={props.form.hostname}
        placeholder="example.com"
        onChange={(hostname) => update({ hostname })}
      />
      <TextFieldRow
        label={t("Port", "端口")}
        hint={
          errors.port
            ? t("Port must be between 1 and 65535.", "端口需在 1 到 65535 之间。")
            : t("Defaults to 22 when left empty.", "留空时使用 22。")
        }
        value={props.form.port}
        placeholder="22"
        onChange={(port) => update({ port })}
      />
      <TextFieldRow
        label={t("User", "用户名")}
        hint={errors.username ? t("User is required.", "用户名不能为空。") : undefined}
        value={props.form.username}
        placeholder="deploy"
        onChange={(username) => update({ username })}
      />
      <TextFieldRow
        label={t("Private key", "私钥")}
        hint={t(
          "Path to the private key file. Leave empty to try the default keys under ~/.ssh.",
          "私钥文件路径。留空则依次尝试 ~/.ssh 下的默认私钥。"
        )}
        value={props.form.identityFile}
        placeholder="~/.ssh/id_ed25519"
        onChange={(identityFile) => update({ identityFile })}
      />
      <TextFieldRow
        label={t("Directory", "登录目录")}
        hint={t("Directory to enter after login. Leave empty to use the remote default.", "登录后进入的目录，留空则使用远端默认目录。")}
        value={props.form.remoteDirectory}
        placeholder="/srv/app"
        onChange={(remoteDirectory) => update({ remoteDirectory })}
      />
    </div>
  );
}
