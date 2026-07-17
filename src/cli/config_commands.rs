use super::*;

pub(super) async fn run_config(paths: &SaiPaths, args: ConfigArgs) -> Result<()> {
    match args.command {
        Some(ConfigCommand::Validate) => {
            AppConfig::load(paths)?;
            println!(
                "{}: {}",
                t("config is valid", "配置有效"),
                paths.config_file.display()
            );
            Ok(())
        }
        Some(ConfigCommand::Paths) => {
            paths.print();
            Ok(())
        }
        Some(ConfigCommand::PromptSource) => {
            let config = AppConfig::load(paths)?;
            let persona = config.prompt.active_persona.trim();
            let identity = config.prompt.active_identity.trim();
            println!(
                "base_prompt_source: {}",
                if persona.is_empty() {
                    "built-in"
                } else {
                    "persona"
                }
            );
            println!(
                "active_persona: {}",
                if persona.is_empty() { "Sai" } else { persona }
            );
            if !persona.is_empty() {
                println!(
                    "active_persona_file: {}",
                    config.persona_path(paths, persona).display()
                );
            }
            println!(
                "active_identity: {}",
                if identity.is_empty() {
                    "(none)"
                } else {
                    identity
                }
            );
            println!("prompts_dir: {}", config.prompts_dir_path(paths).display());
            println!(
                "identities_dir: {}",
                config.identities_dir_path(paths).display()
            );
            let system_prompt = config.system_prompt(paths)?;
            println!(
                "system_prompt_first_line: {}",
                system_prompt.lines().next().unwrap_or("")
            );
            println!("system_prompt_chars: {}", system_prompt.chars().count());
            Ok(())
        }
        None => crate::config_tui::run(paths),
    }
}
