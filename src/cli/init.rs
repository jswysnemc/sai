use super::*;

pub(super) enum InitKind {
    FirstRun,
    Explicit,
}

pub(super) fn run_init(paths: &SaiPaths, kind: InitKind) -> Result<()> {
    let interactive = io::stdin().is_terminal() && io::stdout().is_terminal();
    if interactive {
        println!(
            "{}\n",
            match kind {
                InitKind::FirstRun => t("Sai first start", "Sai 首次启动"),
                InitKind::Explicit => t("Sai initialization", "Sai 初始化"),
            }
        );
    }
    print_init_step(
        interactive,
        t("Preparing config directory", "正在准备配置目录"),
        &paths.config_dir.display().to_string(),
    )?;
    AppConfig::init_files(paths)?;
    print_init_step(
        interactive,
        t("Writing default config", "正在写入默认配置"),
        &paths.config_file.display().to_string(),
    )?;
    print_init_step(
        interactive,
        t("Creating state files", "正在创建状态文件"),
        &paths.state_dir.display().to_string(),
    )?;
    StateStore::new(paths)?.init_files()?;
    let _config = AppConfig::load_or_default(paths)?;
    print_init_step(
        interactive,
        t("Preparing data directory", "正在准备数据目录"),
        &paths.data_dir.display().to_string(),
    )?;
    if interactive {
        println!("\n{}\n", t("Initialization complete.", "初始化完成。"));
        std::thread::sleep(Duration::from_millis(420));
        prompt_shell_init_menu(paths)?;
    } else {
        println!(
            "{} {}",
            t("initialized Sai at", "Sai 已初始化于"),
            paths.config_dir.display()
        );
    }
    Ok(())
}

fn print_init_step(interactive: bool, label: &str, value: &str) -> Result<()> {
    if interactive {
        std::thread::sleep(Duration::from_millis(180));
        println!("  {label:<24} ✓ {value}");
        io::stdout().flush()?;
    }
    Ok(())
}

fn prompt_shell_init_menu(paths: &SaiPaths) -> Result<()> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Ok(());
    }
    println!("{}", t("Integrate with shell?", "是否集成到 shell？"));
    println!(
        "{}\n",
        t(
            "After integration, you can chat in natural language directly in the terminal.",
            "集成后可在终端直接使用自然语言交流。"
        )
    );
    match select_shell_hook()? {
        Some("fish") => shell::fish::install(paths),
        Some("bash") => shell::bash::install(paths),
        Some("zsh") => shell::zsh::install(paths),
        Some("powershell") => shell::powershell::install(paths),
        _ => Ok(()),
    }
}

fn select_shell_hook() -> Result<Option<&'static str>> {
    let options = [
        (t("Skip", "跳过"), None),
        ("fish", Some("fish")),
        ("bash", Some("bash")),
        ("zsh", Some("zsh")),
        ("powershell", Some("powershell")),
    ];
    let detected = shell::current_parent_shell();
    #[cfg(windows)]
    let detected = detected.or_else(|| Some("powershell".to_string()));
    let mut selected = detected
        .as_deref()
        .and_then(|shell| options.iter().position(|(_, value)| *value == Some(shell)))
        .unwrap_or(0);
    let mut stdout = io::stdout();
    let (_, menu_row) = cursor::position()?;
    execute!(stdout, Hide)?;
    struct ShellMenuGuard;
    impl Drop for ShellMenuGuard {
        fn drop(&mut self) {
            let _ = terminal::disable_raw_mode();
            let _ = execute!(io::stdout(), Show);
        }
    }
    let _guard = ShellMenuGuard;
    loop {
        queue!(stdout, MoveTo(0, menu_row))?;
        for (index, (label, _)) in options.iter().enumerate() {
            queue!(stdout, Clear(ClearType::CurrentLine))?;
            if index == selected {
                queue!(stdout, Print(format!("> {label}\n")))?;
            } else {
                queue!(stdout, Print(format!("  {label}\n")))?;
            }
        }
        println!(
            "\n\x1b[2m{}\x1b[0m",
            t(
                "Up/Down or j/k to choose, Enter to confirm, Esc/q to skip",
                "↑/↓ 或 j/k 选择，Enter 确认，Esc/q 跳过"
            )
        );
        stdout.flush()?;
        terminal::enable_raw_mode()?;
        let key = read_shell_menu_key();
        terminal::disable_raw_mode()?;
        match key? {
            KeyCode::Esc | KeyCode::Char('q') => {
                execute!(stdout, Show)?;
                return Ok(None);
            }
            KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => selected = (selected + 1).min(options.len() - 1),
            KeyCode::Enter => {
                execute!(stdout, Show)?;
                return Ok(options[selected].1);
            }
            _ => {}
        }
    }
}

fn read_shell_menu_key() -> Result<KeyCode> {
    loop {
        if let Event::Key(KeyEvent { code, .. }) = event::read()? {
            return Ok(code);
        }
    }
}

pub(super) fn remove_shell_hooks(paths: &SaiPaths) -> Result<()> {
    let removed = shell::fish::uninstall(paths)?;
    let removed = shell::bash::uninstall(paths)? || removed;
    let removed = shell::zsh::uninstall(paths)? || removed;
    shell::powershell::uninstall(paths)?;
    if !removed {
        println!(
            "{}",
            t(
                "no installed Sai shell hooks found",
                "未找到已安装的 Sai shell hook"
            )
        );
    }
    Ok(())
}
