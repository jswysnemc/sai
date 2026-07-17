import { detectInitialLocale, type Locale } from "../features/i18n/locale";

/**
 * 保存服务端原始错误，并在读取 message 时按当前 Web 语言转换固定错误文案。
 */
export class ApiError extends Error {
  readonly rawMessage: string;

  /**
   * 创建可动态本地化的 API 错误。
   *
   * @param rawMessage 服务端返回的原始错误
   */
  constructor(rawMessage: string) {
    super();
    this.name = "ApiError";
    this.rawMessage = rawMessage;
    Object.defineProperty(this, "message", {
      configurable: true,
      get: () => localizeApiErrorMessage(this.rawMessage, detectInitialLocale())
    });
  }
}

/**
 * 将服务端固定英文错误转换为当前界面语言。
 *
 * @param message 服务端原始错误
 * @param locale 当前界面语言
 * @returns 本地化错误；未知或第三方错误保持原文
 */
export function localizeApiErrorMessage(message: string, locale: Locale): string {
  if (locale === "en-US" || !message) return message;

  const exact: Record<string, string> = {
    "no workspace roots are available": "没有可用的工作区根目录",
    "no readable workspace roots are configured": "未配置可读取的工作区根目录",
    "directory name is empty": "目录名称不能为空",
    "directory name contains illegal characters": "目录名称包含非法字符",
    "directory is outside configured workspace roots": "目录不在配置的工作区根目录内",
    "path is not a directory": "路径不是目录",
    "active workspace is missing": "当前工作区不存在",
    "active workspace cannot be removed": "不能移除当前工作区",
    "workspace name cannot be empty": "工作区名称不能为空",
    "invalid web workspace registry": "网页工作区注册表无效",
    "invalid Sai configuration": "Sai 配置无效",
    "prompt name cannot be empty": "提示词名称不能为空",
    "prompt name contains invalid path characters": "提示词名称包含非法路径字符",
    "branch name cannot be empty": "分支名称不能为空",
    "path cannot be empty": "路径不能为空",
    "invalid path": "路径无效",
    "path escapes repository": "路径超出仓库范围",
    "git command failed": "Git 命令执行失败",
    "discard requires at least one path": "丢弃改动时至少需要一个路径",
    "commit sha cannot be empty": "提交 SHA 不能为空",
    "unable to parse commit details": "无法解析提交详情",
    "commit message cannot be empty": "提交说明不能为空",
    "terminal title cannot be empty": "终端标题不能为空",
    "message cannot be empty": "消息不能为空",
    "tree path is not a directory": "文件树路径不是目录",
    "path is not a file": "路径不是文件",
    "binary files are not supported": "不支持二进制文件",
    "file is not valid UTF-8": "文件不是有效的 UTF-8 文本",
    "file is not a supported image": "文件不是支持的图片格式",
    "path is not a regular file": "路径不是常规文件",
    "file path has no parent": "文件路径没有父目录",
    "path already exists": "路径已经存在",
    "target path already exists": "目标路径已经存在",
    "path escapes workspace root": "路径超出工作区根目录",
    "workspace root cannot be modified": "不能修改工作区根目录",
    "absolute paths are not allowed": "不允许使用绝对路径",
    "parent path components are not allowed": "不允许使用父目录路径片段",
    "absolute path is outside the workspace root": "绝对路径不在工作区根目录内",
    "provider_id and model must be provided together": "provider_id 和 model 必须同时提供",
    "provider_id cannot be empty": "provider_id 不能为空",
    "provider_id and model cannot be empty": "provider_id 和 model 不能为空"
  };
  if (exact[message]) return exact[message];

  const patterns: Array<[RegExp, (match: RegExpMatchArray) => string]> = [
    [/^directory already exists: (.+)$/u, (match) => `目录已经存在：${match[1]}`],
    [/^failed to create directory: (.+)$/u, (match) => `创建目录失败：${match[1]}`],
    [/^directory does not exist: (.+)$/u, (match) => `目录不存在：${match[1]}`],
    [/^path is not a directory: (.+)$/u, (match) => `路径不是目录：${match[1]}`],
    [/^workspace not found: (.+)$/u, (match) => `未找到工作区：${match[1]}`],
    [/^workspace does not exist: (.+)$/u, (match) => `工作区不存在：${match[1]}`],
    [/^failed to enter workspace (.+)$/u, (match) => `进入工作区失败：${match[1]}`],
    [/^prompt not found: (.+)$/u, (match) => `未找到提示词：${match[1]}`],
    [/^terminal not found: (.+)$/u, (match) => `未找到终端：${match[1]}`],
    [/^failed to start terminal shell (.+)$/u, (match) => `启动终端 Shell 失败：${match[1]}`],
    [/^path does not exist: (.+)$/u, (match) => `路径不存在：${match[1]}`],
    [/^parent directory does not exist: (.+)$/u, (match) => `父目录不存在：${match[1]}`],
    [/^file exceeds (.+)$/u, (match) => `文件超过大小限制：${match[1]}`],
    [/^image exceeds (.+)$/u, (match) => `图片超过大小限制：${match[1]}`],
    [/^unsupported prompt kind: (.+)$/u, (match) => `不支持的提示词类型：${match[1]}`],
    [/^unsupported git action: (.+)$/u, (match) => `不支持的 Git 操作：${match[1]}`],
    [/^unsupported run mode: (.+)$/u, (match) => `不支持的运行模式：${match[1]}`],
    [/^unsupported thinking level: (.+)$/u, (match) => `不支持的思考等级：${match[1]}`],
    [/^provider not found: (.+)$/u, (match) => `未找到 provider：${match[1]}`]
  ];
  for (const [pattern, translate] of patterns) {
    const match = message.match(pattern);
    if (match) return translate(match);
  }
  return message;
}
