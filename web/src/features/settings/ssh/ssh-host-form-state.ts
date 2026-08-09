import type { SshHost, SshHostInput } from "../../../api/contracts";

export const DEFAULT_SSH_PORT = 22;

/** SSH 主机表单的编辑态 */
export type SshHostFormState = {
  label: string;
  hostname: string;
  /** 端口以文本保存，便于处理清空与非法输入 */
  port: string;
  username: string;
  identityFile: string;
  remoteDirectory: string;
};

/** 表单逐字段的校验结果，字段缺席表示通过 */
export type SshHostFormErrors = Partial<Record<"hostname" | "port" | "username", string>>;

export const EMPTY_SSH_HOST_FORM: SshHostFormState = {
  label: "",
  hostname: "",
  port: String(DEFAULT_SSH_PORT),
  username: "",
  identityFile: "",
  remoteDirectory: ""
};

/**
 * 把已保存的主机配置转成表单编辑态。
 *
 * @param host 已保存的主机配置
 * @returns 表单编辑态
 */
export function toSshHostForm(host: SshHost): SshHostFormState {
  return {
    label: host.label,
    hostname: host.hostname,
    port: String(host.port),
    username: host.username,
    identityFile: host.identity_file,
    remoteDirectory: host.remote_directory
  };
}

/**
 * 校验主机表单。
 *
 * @param form 表单编辑态
 * @returns 逐字段错误信息，全部通过时为空对象
 */
export function validateSshHostForm(form: SshHostFormState): SshHostFormErrors {
  const errors: SshHostFormErrors = {};
  if (!form.hostname.trim()) errors.hostname = "required";
  if (!form.username.trim()) errors.username = "required";

  // 端口留空按默认端口处理，填写则必须是 1-65535 的整数
  const port = form.port.trim();
  if (port) {
    const parsed = Number(port);
    if (!Number.isInteger(parsed) || parsed < 1 || parsed > 65535) errors.port = "range";
  }
  return errors;
}

/**
 * 判断表单是否可以提交。
 *
 * @param form 表单编辑态
 * @returns 无校验错误时返回 true
 */
export function canSubmitSshHostForm(form: SshHostFormState): boolean {
  return Object.keys(validateSshHostForm(form)).length === 0;
}

/**
 * 把表单编辑态转成提交给后端的载荷。
 *
 * 标签留空时回落为主机名，端口留空时回落为默认端口。
 *
 * @param form 表单编辑态
 * @returns 提交载荷
 */
export function toSshHostInput(form: SshHostFormState): SshHostInput {
  const hostname = form.hostname.trim();
  const port = Number(form.port.trim());
  return {
    label: form.label.trim() || hostname,
    hostname,
    port: Number.isInteger(port) && port >= 1 && port <= 65535 ? port : DEFAULT_SSH_PORT,
    username: form.username.trim(),
    identity_file: form.identityFile.trim(),
    remote_directory: form.remoteDirectory.trim()
  };
}

/**
 * 组合主机的展示地址。
 *
 * @param host 主机配置
 * @returns user@host 或 user@host:port
 */
export function sshHostAddress(host: Pick<SshHost, "username" | "hostname" | "port">): string {
  const base = `${host.username}@${host.hostname}`;
  return host.port === DEFAULT_SSH_PORT ? base : `${base}:${host.port}`;
}
