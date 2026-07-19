use super::background_commands::BackgroundCommandsArgs;
use crate::gateways::cli::{GatewayArgs, WeixinLoginArgs};
use crate::i18n::{text as t, Locale};
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "sai", version, about = "Sai CLI AI Agent")]
pub struct Cli {
    #[arg(long, global = true, value_name = "LANG", value_parser = parse_language_argument)]
    pub lang: Option<String>,

    #[arg(long)]
    pub plan: bool,

    /// 启用带审计日志和工作区沙盒的执行模式
    #[arg(long, conflicts_with_all = ["plan", "yolo"])]
    pub audited: bool,

    /// 显式启用不询问权限的执行模式
    #[arg(long, conflicts_with_all = ["plan", "audited"])]
    pub yolo: bool,

    #[arg(short = 'c', long = "clipb")]
    pub clipb: bool,

    #[arg(short = 'w', long = "web")]
    pub web_search: bool,

    #[arg(short = 'e', long = "explain")]
    pub explain: bool,

    #[arg(long, value_name = "LEVEL")]
    pub thinking: Option<String>,

    #[arg(long, hide = true)]
    pub shell_intercept: bool,

    #[arg(long, hide = true)]
    pub shell: Option<String>,

    #[command(subcommand)]
    pub command: Option<Command>,

    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub message: Vec<String>,
}

/// 校验命令行界面语言并返回标准语言代码。
///
/// 参数:
/// - `value`: 用户输入的语言代码
///
/// 返回:
/// - 规范化后的语言代码；不受支持时返回校验错误
fn parse_language_argument(value: &str) -> Result<String, String> {
    Locale::parse(value)
        .map(|locale| locale.code().to_string())
        .ok_or_else(|| {
            t(
                "language must be en-US or zh-CN",
                "语言必须为 en-US 或 zh-CN",
            )
            .to_string()
        })
}

#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(name = "__alarm-worker", hide = true)]
    AlarmWorker(AlarmWorkerArgs),
    #[command(name = "__tool", hide = true)]
    Tool(ToolArgs),
    /// 启动 Sai Web 编程工作台
    Web(WebArgs),
    Ask(MessageArgs),
    Init,
    Paths,
    Config(ConfigArgs),
    Providers(ProvidersArgs),
    FishInit,
    BashInit,
    ZshInit,
    PowershellInit,
    RemoveShellHook,
    History(HistoryArgs),
    #[command(alias = "session")]
    Sessions(SessionsArgs),
    /// 交互选择或按 ID 恢复会话
    Resume(ResumeArgs),
    Kb(KbArgs),
    Memory(MemoryArgs),
    Skills(SkillsArgs),
    Ps(BackgroundCommandsArgs),
    Gateway(GatewayArgs),
    WeixinLogin(TopLevelWeixinLoginArgs),
    Set(SetArgs),
    Clear(ClearArgs),
    Compact(CompactArgs),
}

#[derive(Debug, Args)]
pub struct TopLevelWeixinLoginArgs {
    #[arg(long, short = 'v')]
    pub verbose: bool,

    #[command(flatten)]
    pub login: WeixinLoginArgs,
}

#[derive(Debug, Args)]
pub struct MessageArgs {
    #[arg(short = 'c', long = "clipb")]
    pub clipb: bool,

    #[arg(short = 'w', long = "web")]
    pub web_search: bool,

    #[arg(long, value_name = "LEVEL")]
    pub thinking: Option<String>,

    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub message: Vec<String>,
}

#[derive(Debug, Args)]
pub struct ClearArgs {
    #[arg(long, conflicts_with = "scope")]
    pub memory: bool,

    pub scope: Option<String>,
}

#[derive(Debug, Args)]
pub struct CompactArgs {}

#[derive(Debug, Args)]
pub struct SetArgs {
    #[command(subcommand)]
    pub command: SetCommand,
}

#[derive(Debug, Subcommand)]
pub enum SetCommand {
    Thinking(SetThinkingArgs),
}

#[derive(Debug, Args)]
pub struct SetThinkingArgs {
    pub level: Option<String>,
}

#[derive(Debug, Args)]
pub struct AlarmWorkerArgs {
    #[arg(long)]
    pub id: String,
    #[arg(long)]
    pub time: String,
    #[arg(long, default_value = "Sai alarm")]
    pub label: String,
    #[arg(long)]
    pub state_dir: PathBuf,
    #[arg(long)]
    pub audio_file: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct ToolArgs {
    pub name: String,
    pub arguments: Option<String>,
}

#[derive(Debug, Args)]
pub struct WebArgs {
    #[arg(long, visible_alias = "prot", default_value_t = 4096)]
    pub port: u16,

    #[arg(long)]
    pub no_open: bool,

    #[arg(long, value_name = "PATH")]
    pub workspace: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: Option<ConfigCommand>,
}

#[derive(Debug, Args)]
pub struct HistoryArgs {
    #[arg(short, long, default_value_t = 20)]
    pub limit: usize,

    #[arg(long)]
    pub raw: bool,

    #[arg(long)]
    pub no_thinking: bool,
}

#[derive(Debug, Args)]
pub struct SessionsArgs {
    #[command(subcommand)]
    pub command: Option<SessionsCommand>,
}

#[derive(Debug, Subcommand)]
pub enum SessionsCommand {
    List,
    New(SessionTitleArgs),
    Switch(SessionIdArgs),
    /// 交互选择或按 ID 恢复会话
    Resume(ResumeArgs),
    Current,
    Delete(SessionIdArgs),
    Rename(SessionRenameArgs),
}

#[derive(Debug, Args)]
pub struct ResumeArgs {
    /// 可选会话 ID；省略时进入交互选择
    pub id: Option<String>,
}

#[derive(Debug, Args)]
pub struct SessionTitleArgs {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub title: Vec<String>,
}

#[derive(Debug, Args)]
pub struct SessionIdArgs {
    pub id: String,
}

#[derive(Debug, Args)]
pub struct SessionRenameArgs {
    pub id: String,
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub title: Vec<String>,
}

#[derive(Debug, Args)]
pub struct ProvidersArgs {
    pub index: Option<usize>,
}

#[derive(Debug, Args)]
pub struct KbArgs {
    #[command(subcommand)]
    pub command: KbCommand,
}

#[derive(Debug, Args)]
pub struct MemoryArgs {
    #[command(subcommand)]
    pub command: MemoryCommand,
}

#[derive(Debug, Subcommand)]
pub enum MemoryCommand {
    Stats,
    Reset(MemoryResetArgs),
    Search(MemorySearchArgs),
    Remember(MemoryRememberArgs),
}

#[derive(Debug, Args)]
pub struct MemoryResetArgs {
    #[arg(long)]
    pub include_skills: bool,
}

#[derive(Debug, Args)]
pub struct MemorySearchArgs {
    pub query: Vec<String>,
    #[arg(short, long)]
    pub limit: Option<usize>,
    #[arg(long)]
    pub forgotten: bool,
}

#[derive(Debug, Args)]
pub struct MemoryRememberArgs {
    pub content: Vec<String>,
    #[arg(short, long, default_value = "manual")]
    pub source: String,
}

#[cfg(test)]
mod tests {
    use super::{ClearArgs, Cli, Command};
    use clap::Parser;
    use std::path::PathBuf;

    /// 验证 clear 命令可仅清空助手记忆。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn parses_clear_memory_flag() {
        let cli = Cli::try_parse_from(["sai", "clear", "--memory"]).unwrap();

        assert!(matches!(
            cli.command,
            Some(Command::Clear(ClearArgs {
                memory: true,
                scope: None
            }))
        ));
    }

    /// 验证可以显式覆盖配置中的默认权限模式为 YOLO。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn parses_explicit_yolo_mode() {
        let cli = Cli::try_parse_from(["sai", "--yolo", "inspect"]).unwrap();

        assert!(cli.yolo);
        assert!(!cli.plan);
        assert!(!cli.audited);
    }

    /// 验证权限模式覆盖参数互斥。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn rejects_conflicting_permission_mode_flags() {
        let result = Cli::try_parse_from(["sai", "--yolo", "--audited", "inspect"]);

        assert!(result.is_err());
    }

    /// 验证顶层微信登录兼容命令可以正确解析参数。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn parses_top_level_weixin_login_command() {
        let cli = Cli::try_parse_from([
            "sai",
            "weixin-login",
            "--verbose",
            "--bot-type",
            "3",
            "--timeout-secs",
            "30",
        ])
        .unwrap();

        let Some(Command::WeixinLogin(args)) = cli.command else {
            panic!("expected top-level weixin-login command");
        };
        assert!(args.verbose);
        assert_eq!(args.login.bot_type.as_deref(), Some("3"));
        assert_eq!(args.login.timeout_secs, 30);
    }

    /// 验证 Web 子命令可以指定独立进程的初始工作区。
    #[test]
    fn parses_web_workspace() {
        let cli = Cli::try_parse_from([
            "sai",
            "web",
            "--port",
            "0",
            "--no-open",
            "--workspace",
            "/workspace/repository",
        ])
        .unwrap();

        let Some(Command::Web(args)) = cli.command else {
            panic!("expected web command");
        };
        assert_eq!(args.port, 0);
        assert!(args.no_open);
        assert_eq!(args.workspace, Some(PathBuf::from("/workspace/repository")));
    }
}

#[derive(Debug, Args)]
pub struct SkillsArgs {
    #[command(subcommand)]
    pub command: SkillsCommand,
}

#[derive(Debug, Subcommand)]
pub enum SkillsCommand {
    List,
    Show(SkillNameArgs),
    Enable(SkillNameArgs),
    Disable(SkillNameArgs),
    Remove(SkillNameArgs),
    Stats,
    Prune,
}

#[derive(Debug, Args)]
pub struct SkillNameArgs {
    pub name: String,
}

#[derive(Debug, Subcommand)]
pub enum KbCommand {
    Add(KbAddArgs),
    List,
    Search(KbSearchArgs),
    Find(KbFindArgs),
    Read(KbReadArgs),
    Remove(KbRemoveArgs),
    Reindex,
    Stats,
    Embed(KbEmbedArgs),
}

#[derive(Debug, Args)]
pub struct KbAddArgs {
    pub path: PathBuf,
    #[arg(
        short,
        long,
        help = "Compatibility flag; directories are recursive by default"
    )]
    pub recursive: bool,
}

#[derive(Debug, Args)]
pub struct KbSearchArgs {
    pub query: Vec<String>,
    #[arg(short, long)]
    pub limit: Option<usize>,
}

#[derive(Debug, Args)]
pub struct KbFindArgs {
    pub query: Vec<String>,
    #[arg(short, long)]
    pub limit: Option<usize>,
}

#[derive(Debug, Args)]
pub struct KbReadArgs {
    pub file: String,
    #[arg(long, default_value_t = 1)]
    pub start: usize,
    #[arg(long)]
    pub lines: Option<usize>,
}

#[derive(Debug, Args)]
pub struct KbRemoveArgs {
    pub file: String,
}

#[derive(Debug, Args)]
pub struct KbEmbedArgs {
    #[command(subcommand)]
    pub command: KbEmbedCommand,
}

#[derive(Debug, Subcommand)]
pub enum KbEmbedCommand {
    Reindex(KbEmbedReindexArgs),
}

#[derive(Debug, Args)]
pub struct KbEmbedReindexArgs {
    #[arg(long)]
    pub quiet: bool,
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    Validate,
    Paths,
    #[command(hide = true)]
    PromptSource,
}
