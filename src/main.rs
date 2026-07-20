mod agent;
mod alarm;
mod reply_notify;
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
mod runner;
mod runtime_cwd;
mod runtime_recovery;
mod shell;
mod state;
mod token_counter;
mod token_estimate;
mod usage_history;
mod tools;
mod web;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = cli::parse();
    cli::run(cli).await
}
