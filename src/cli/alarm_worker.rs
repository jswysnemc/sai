use super::*;

pub(super) fn run_alarm_worker(args: AlarmWorkerArgs) -> Result<()> {
    let paths = alarm_worker_paths(args.state_dir);
    let seconds = crate::alarm::parse_alarm_seconds(&args.time)?;
    let source = args
        .audio_file
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "builtin".to_string());
    let _ = append_alarm_log(
        &paths,
        &format!("{}: scheduled in {seconds}s; source={source}\n", args.id),
    );
    std::thread::sleep(Duration::from_secs(seconds));
    let _ = crate::alarm::update_status(&paths, &args.id, crate::alarm::AlarmStatus::Ringing);
    let _ = append_alarm_log(&paths, &format!("{}: playback starting\n", args.id));
    let result = play_alarm_once(args.audio_file.as_deref()).or_else(|err| {
        append_alarm_log(
            &paths,
            &format!("{}: audio playback failed: {err}\n", args.id),
        )?;
        terminal_bell_fallback();
        Ok(())
    });
    if result.is_ok() {
        let _ = append_alarm_log(&paths, &format!("{}: playback finished\n", args.id));
    }
    let _ = crate::alarm::remove(&paths, &args.id);
    result
}

fn play_alarm_once(audio_file: Option<&std::path::Path>) -> Result<()> {
    const ALARM_WAV: &[u8] = include_bytes!("../assets/alarm.wav");
    let (_stream, handle) = rodio::OutputStream::try_default()?;
    let audio = match audio_file {
        Some(path) => std::fs::read(path)?,
        None => ALARM_WAV.to_vec(),
    };
    let cursor = Cursor::new(audio);
    let sink = rodio::Sink::try_new(&handle)?;
    let source = rodio::Decoder::new(cursor)?;
    sink.append(source);
    sink.sleep_until_end();
    Ok(())
}

fn terminal_bell_fallback() {
    for _ in 0..5 {
        let _ = std::io::stderr().write_all(b"\x07");
        let _ = std::io::stderr().flush();
        std::thread::sleep(Duration::from_secs(1));
    }
}

fn append_alarm_log(paths: &SaiPaths, line: &str) -> Result<()> {
    std::fs::create_dir_all(&paths.state_dir)?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(crate::alarm::alarm_log_file(paths))?;
    file.write_all(line.as_bytes())?;
    Ok(())
}

fn alarm_worker_paths(state_dir: PathBuf) -> SaiPaths {
    SaiPaths {
        config_dir: PathBuf::new(),
        config_file: PathBuf::new(),
        secrets_file: PathBuf::new(),
        skills_dir: PathBuf::new(),
        data_dir: PathBuf::new(),
        cache_dir: PathBuf::new(),
        state_dir,
        pictures_dir: PathBuf::new(),
        fish_hook_file: PathBuf::new(),
        bash_hook_file: PathBuf::new(),
        zsh_hook_file: PathBuf::new(),
        powershell_hook_file: PathBuf::new(),
    }
}
