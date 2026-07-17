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
      get: () => localizeApiMessage(this.rawMessage, detectInitialLocale())
    });
  }
}

/**
 * 保存双语兜底错误，并在读取 message 时采用当前 Web 语言。
 */
export class LocalizedError extends Error {
  readonly englishMessage: string;
  readonly chineseMessage: string;

  /**
   * 创建可动态切换语言的界面错误。
   *
   * @param englishMessage 英文错误文案
   * @param chineseMessage 中文错误文案
   */
  constructor(englishMessage: string, chineseMessage: string) {
    super();
    this.name = "LocalizedError";
    this.englishMessage = englishMessage;
    this.chineseMessage = chineseMessage;
    Object.defineProperty(this, "message", {
      configurable: true,
      get: () => detectInitialLocale() === "zh-CN" ? this.chineseMessage : this.englishMessage
    });
  }
}

/**
 * 将捕获的异常转换为可存入组件状态的错误对象。
 *
 * @param cause 捕获的异常
 * @param englishFallback 非错误对象对应的英文兜底文案
 * @param chineseFallback 非错误对象对应的中文兜底文案
 * @returns 保留动态 message 的原错误，或可动态切换语言的兜底错误
 */
export function toDisplayError(
  cause: unknown,
  englishFallback: string,
  chineseFallback: string
): Error {
  return cause instanceof Error
    ? cause
    : new LocalizedError(englishFallback, chineseFallback);
}

/**
 * 将服务端固定英文消息转换为当前界面语言。
 *
 * @param message 服务端原始消息
 * @param locale 当前界面语言
 * @returns 本地化消息；未知或第三方消息保持原文
 */
export function localizeApiMessage(message: string, locale: Locale): string {
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
    "provider_id and model cannot be empty": "provider_id 和 model 不能为空",
    "answers are required unless cancelled": "未取消提问时必须提供答案",
    "unable to find a base branch for review; set an upstream or fetch the main branch first": "找不到可用于审查的基线分支，请先设置 upstream 或 fetch 主分支",
    "no staged changes to commit": "没有已暂存的改动可提交",
    "repository has no remote configured": "当前仓库还没有设置远端仓库",
    "remote URL cannot be empty": "远端仓库地址不能为空",
    "not on a local branch that can be pulled": "当前不在可拉取的本地分支上",
    "current branch has no upstream and origin remote is unavailable": "当前分支没有 upstream，且找不到 origin remote",
    "not on a local branch that can be pushed": "当前不在可推送的本地分支上",
    "current directory is not a Git repository": "当前目录不是 Git 仓库",
    "repository initialized": "仓库已初始化",
    "files staged": "文件已暂存",
    "files unstaged": "文件已取消暂存",
    "changes discarded": "改动已放弃",
    "commit created": "提交已创建",
    "fetch completed": "Fetch 完成",
    "pull completed": "Pull 完成",
    "push completed": "Push 完成",
    "remote repository saved": "远端仓库已保存",
    "branch switched": "分支已切换",
    "branch created": "分支已创建",
    "path added to .gitignore": "路径已添加到 .gitignore",
    "changes stashed": "改动已贮藏",
    "stash popped": "贮藏已弹出",
    "operation completed": "操作完成",
    "Gateway sessions": "网关会话",
    "model endpoint returned no result": "模型接口未返回结果",
    "conversation has been compacted multiple times; start a focused session if details become distorted": "当前会话已经多次压缩；如果细节开始失真，请新建聚焦会话继续",
    "verification code submitted; awaiting confirmation": "验证码已提交，正在确认",
    "QR code scanned; confirm on your phone": "已扫码，等待手机确认",
    "verification code required": "需要输入验证码",
    "verification code rejected; enter it again": "验证码被拒绝，请重新输入",
    "QR code expired; request a new one": "二维码已过期，请重新获取",
    "this Weixin account is already linked, but no new credentials were returned; start the service directly if the account is saved locally": "该微信账号已绑定，但未返回新凭证；如本机已保存账号可直接启动服务",
    "login succeeded but credentials are missing": "登录成功但响应缺少凭证",
    "login successful": "登录成功",
    "login timed out; request a new QR code": "登录超时，请重新获取二维码"
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
    [/^provider not found: (.+)$/u, (match) => `未找到 provider：${match[1]}`],
    [/^failed to read \.gitignore: (.+)$/u, (match) => `读取 .gitignore 失败：${match[1]}`],
    [/^unknown login status: (.+)$/u, (match) => `未知登录状态：${match[1]}`],
    [/^failed to save credentials: (.+)$/u, (match) => `凭证保存失败：${match[1]}`]
  ];
  for (const [pattern, translate] of patterns) {
    const match = message.match(pattern);
    if (match) return translate(match);
  }
  return message;
}

/**
 * 将服务端固定英文错误转换为当前界面语言。
 *
 * @param message 服务端原始错误
 * @param locale 当前界面语言
 * @returns 本地化错误；未知或第三方错误保持原文
 */
export function localizeApiErrorMessage(message: string, locale: Locale): string {
  return localizeApiMessage(message, locale);
}
