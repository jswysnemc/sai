import { useI18n } from "../../i18n/use-i18n";
import { FieldGrid, SettingsField, SkTextInput } from "../kit";
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

  const fields = [
    { key: "label", en: "Name", zh: "名称", hint: t("Shown in the terminal target list; defaults to the hostname.", "展示在终端目标列表中，留空则使用主机名。") },
    { key: "hostname", en: "Host", zh: "主机", error: errors.hostname ? t("Host is required.", "主机不能为空。") : undefined },
    { key: "port", en: "Port", zh: "端口", hint: t("Defaults to 22 when empty.", "留空时使用 22。"), error: errors.port ? t("Port must be between 1 and 65535.", "端口需在 1 到 65535 之间。") : undefined },
    { key: "username", en: "User", zh: "用户名", error: errors.username ? t("User is required.", "用户名不能为空。") : undefined },
    { key: "identityFile", en: "Private key", zh: "私钥", hint: t("Path on the server; leave empty to try default keys under ~/.ssh.", "服务端私钥路径；留空尝试 ~/.ssh 下的默认私钥。") },
    { key: "remoteDirectory", en: "Directory", zh: "登录目录", hint: t("Directory to enter after login; empty uses the remote default.", "登录后进入的目录，留空使用远端默认目录。") }
  ] as const;
  return <FieldGrid>{fields.map((field) => <SettingsField key={field.key} label={t(field.en, field.zh)} anchor={`ssh.${field.key}`} size={field.key === "port" ? "sm" : "full"} hint={"hint" in field ? field.hint : undefined} error={"error" in field ? field.error : undefined}>
    <SkTextInput value={props.form[field.key]} inputMode={field.key === "port" ? "numeric" : undefined} onChange={(value) => update({ [field.key]: value })} />
  </SettingsField>)}</FieldGrid>;
}
