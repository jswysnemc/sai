use super::args::WebPasswordCommand;
use crate::config::SecretsConfig;
use crate::paths::SaiPaths;
use crate::web::password::{hash_web_password, MIN_WEB_PASSWORD_LENGTH};
use anyhow::{Context, Result};

/// 执行 Web 访问口令管理命令。
///
/// 口令只以 Argon2 哈希形式写入 secrets 文件（0600 权限），明文不落盘。
///
/// 参数:
/// - `paths`: Sai 路径集合
/// - `command`: 子命令
///
/// 返回:
/// - 执行结果
pub(crate) fn run(paths: &SaiPaths, command: WebPasswordCommand) -> Result<()> {
    let mut secrets = SecretsConfig::load(paths)?;
    match command {
        WebPasswordCommand::Status => {
            if secrets.web_password_hash.is_some() {
                println!("Sai Web password: set");
            } else {
                println!("Sai Web password: not set (start token only)");
            }
        }
        WebPasswordCommand::Clear => {
            if secrets.web_password_hash.take().is_none() {
                println!("Sai Web password was not set.");
                return Ok(());
            }
            secrets.save(paths)?;
            println!("Sai Web password cleared.");
        }
        WebPasswordCommand::Set { password } => {
            // 1. 未通过参数给出口令时从终端读取，避免进入 shell 命令历史
            let password = match password {
                Some(password) => password,
                None => prompt_password()?,
            };
            secrets.web_password_hash = Some(hash_web_password(&password)?);
            secrets.save(paths)?;
            println!("Sai Web password updated.");
        }
    }
    Ok(())
}

/// 从终端读取口令并要求重复确认。
///
/// 输入期间关闭终端回显，口令不显示在屏幕上，也不进入 shell 历史。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 两次输入一致的口令
fn prompt_password() -> Result<String> {
    let password = read_hidden_line(&format!(
        "New Sai Web password (at least {MIN_WEB_PASSWORD_LENGTH} characters): "
    ))?;
    let confirmation = read_hidden_line("Repeat password: ")?;
    if password != confirmation {
        anyhow::bail!("the two passwords do not match");
    }
    Ok(password)
}

/// 关闭回显读取一行输入。
///
/// 参数:
/// - `prompt`: 提示文本
///
/// 返回:
/// - 去除换行后的输入内容
fn read_hidden_line(prompt: &str) -> Result<String> {
    use std::io::{BufRead, Write};

    print!("{prompt}");
    std::io::stdout().flush()?;

    // 1. 进入原始模式关闭回显，读取期间终端不回显按键
    crossterm::terminal::enable_raw_mode().context("failed to switch the terminal to raw mode")?;
    let mut line = String::new();
    let result = std::io::stdin().lock().read_line(&mut line);
    // 2. 无论读取成败都要恢复终端模式，否则后续命令行不可用
    let _ = crossterm::terminal::disable_raw_mode();
    println!();

    result.context("failed to read the password from the terminal")?;
    Ok(line.trim_end_matches(['\r', '\n']).to_string())
}
