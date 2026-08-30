mod acp;
mod agent;
mod agent_engine;
mod alarm;
mod assistants;
mod cli;
mod clipboard;
mod config;
mod config_tui;
mod control_commands;
mod cron;
mod default_models;
mod gateways;
mod goal;
mod hooks;
mod i18n;
mod ipc;
mod llm;
mod mcp;
mod memory;
mod paths;
mod perf_trace;
mod permission;
mod platform;
mod prompts;
mod question;
mod question_tui;
mod render;
mod reply_notify;
mod runner;
mod runtime_cwd;
mod runtime_recovery;
mod shell;
mod ssh;
mod state;
mod token_counter;
mod token_estimate;
mod tools;
mod usage_history;
mod web;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = cli::parse();
    match cli::run(cli).await {
        Ok(()) => Ok(()),
        // 已展示过的错误只透传退出码，避免 stderr 上重复打印错误链
        Err(err) => match err.downcast_ref::<cli::SilentExit>() {
            Some(exit) => std::process::exit(exit.code),
            None => Err(err),
        },
    }
}
