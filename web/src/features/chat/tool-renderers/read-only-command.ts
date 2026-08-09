/**
 * 只读 shell 命令检测。
 *
 * 供工具分组判断「这条命令是不是在读东西」：cat、ls、grep 之类的
 * 读取型命令与 read_file、grep 工具同属探索行为，总览时应归入同一类。
 * 检测是保守的启发式——按连接符切段后逐段核对白名单，
 * 拿不准的一律判为非只读，宁可漏归类也不把写操作说成探索。
 */

/** 无副作用的读取/查询类命令名单 */
const READ_ONLY_COMMANDS = new Set([
  "cat",
  "ls",
  "head",
  "tail",
  "less",
  "more",
  "grep",
  "egrep",
  "fgrep",
  "rg",
  "ag",
  "find",
  "fd",
  "wc",
  "stat",
  "file",
  "tree",
  "pwd",
  "which",
  "whereis",
  "type",
  "readlink",
  "realpath",
  "basename",
  "dirname",
  "du",
  "df",
  "diff",
  "cmp",
  "sort",
  "uniq",
  "cut",
  "tr",
  "jq",
  "nl",
  "od",
  "xxd",
  "strings",
  "echo",
  "printf",
  "env",
  "printenv",
  "date",
  "whoami",
  "hostname",
  "uname",
  "cd"
]);

/** git 的只读子命令 */
const GIT_READ_ONLY_SUBCOMMANDS = new Set([
  "status",
  "log",
  "diff",
  "show",
  "branch",
  "blame",
  "rev-parse",
  "remote",
  "describe",
  "shortlog",
  "ls-files",
  "reflog"
]);

/** 不产生写入的常见重定向片段，判定前先剔除 */
const HARMLESS_REDIRECTS = ["2>&1", "&>/dev/null", "2>/dev/null", "> /dev/null", ">/dev/null"];

/**
 * 判断一条 shell 命令是否只做读取。
 *
 * 步骤:
 * 1. 剔除无害重定向后，残留的 `>` 视为写文件，直接判非只读
 * 2. 按 `&&`、`||`、`|`、`;` 切成独立命令段
 * 3. 每段去掉 env 赋值前缀后取首个命令词核对白名单，git 再核对子命令
 *
 * 参数:
 * - `command`: 完整命令文本
 *
 * 返回:
 * - 每一段都命中只读名单时返回 true；空命令或任一段拿不准返回 false
 */
export function isReadOnlyShellCommand(command: string): boolean {
  let normalized = command.replace(/\s+/g, " ").trim();
  if (!normalized) return false;
  for (const redirect of HARMLESS_REDIRECTS) {
    normalized = normalized.replaceAll(redirect, " ");
  }
  if (normalized.includes(">")) return false;
  const segments = normalized
    .split(/&&|\|\||[|;]/)
    .map((segment) => segment.trim())
    .filter(Boolean);
  if (segments.length === 0) return false;
  return segments.every(isReadOnlySegment);
}

/**
 * 判断单个命令段是否只读。
 *
 * 参数:
 * - `segment`: 去掉连接符后的一段命令
 *
 * 返回:
 * - 首个命令词在只读名单内时返回 true
 */
function isReadOnlySegment(segment: string): boolean {
  // 1. 去掉 FOO=bar 形式的 env 赋值前缀
  const withoutEnv = segment.replace(/^(?:[A-Za-z_]\w*=\S*\s+)+/, "");
  const words = withoutEnv.split(" ").filter(Boolean);
  if (words.length === 0) return false;
  // 2. 绝对路径调用按可执行名判定
  const name = words[0].split("/").pop() ?? "";
  if (name === "git") {
    return GIT_READ_ONLY_SUBCOMMANDS.has(gitSubcommand(words));
  }
  return READ_ONLY_COMMANDS.has(name);
}

/**
 * 提取 git 段的子命令词。
 *
 * 参数:
 * - `words`: 以 git 开头的命令词序列
 *
 * 返回:
 * - 跳过 `-C <dir>` 与 `--xx` 全局选项后的首个子命令；缺失时返回空串
 */
function gitSubcommand(words: string[]): string {
  let index = 1;
  while (index < words.length) {
    if (words[index] === "-C" || words[index] === "-c") {
      index += 2;
      continue;
    }
    if (words[index].startsWith("-")) {
      index += 1;
      continue;
    }
    return words[index];
  }
  return "";
}
