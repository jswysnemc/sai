/** SSH 主机配置，凭据只保存私钥路径 */
export type SshHost = {
  id: string;
  label: string;
  hostname: string;
  port: number;
  username: string;
  identity_file: string;
  remote_directory: string;
};

/** 新增或修改主机时提交的表单内容 */
export type SshHostInput = {
  label: string;
  hostname: string;
  port: number;
  username: string;
  identity_file: string;
  remote_directory: string;
};

/** 从 ~/.ssh/config 解析出的可导入主机 */
export type SshImportCandidate = {
  label: string;
  hostname: string;
  port: number;
  username: string;
  identity_file: string;
  /** 已存在同地址主机，前端默认不勾选 */
  duplicate: boolean;
};

/** 待用户确认的远端主机密钥 */
export type SshHostKeyPrompt = {
  hostname: string;
  port: number;
  algorithm: string;
  key_base64: string;
  fingerprint: string;
  /** 为真表示已登记密钥与本次不符，可能遭遇中间人攻击 */
  changed: boolean;
};
