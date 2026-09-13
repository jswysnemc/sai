use super::*;

#[cfg(unix)]
#[test]
fn load_repairs_main_config_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let paths = crate::paths::SaiPaths {
        config_dir: root.join("config"),
        config_file: root.join("config/config.jsonc"),
        secrets_file: root.join("config/secrets.jsonc"),
        skills_dir: root.join("config/skills"),
        data_dir: root.join("data"),
        cache_dir: root.join("cache"),
        state_dir: root.join("state"),
        pictures_dir: root.join("pictures"),
        fish_hook_file: root.join("fish/sai.fish"),
        bash_hook_file: root.join("shell/bash-hook.sh"),
        zsh_hook_file: root.join("shell/zsh-hook.zsh"),
        powershell_hook_file: root.join("shell/powershell-hook.ps1"),
    };
    AppConfig::default().save(&paths).unwrap();
    let created_mode = std::fs::metadata(&paths.config_file)
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(created_mode, 0o600);
    std::fs::set_permissions(&paths.config_file, std::fs::Permissions::from_mode(0o644)).unwrap();

    let _ = AppConfig::load(&paths).unwrap();

    let mode = std::fs::metadata(&paths.config_file)
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600);
}

#[cfg(unix)]
#[test]
fn init_files_repairs_credential_permissions_with_invalid_config() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let paths = crate::paths::SaiPaths {
        config_dir: root.join("config"),
        config_file: root.join("config/config.jsonc"),
        secrets_file: root.join("config/secrets.jsonc"),
        skills_dir: root.join("config/skills"),
        data_dir: root.join("data"),
        cache_dir: root.join("cache"),
        state_dir: root.join("state"),
        pictures_dir: root.join("pictures"),
        fish_hook_file: root.join("fish/sai.fish"),
        bash_hook_file: root.join("shell/bash-hook.sh"),
        zsh_hook_file: root.join("shell/zsh-hook.zsh"),
        powershell_hook_file: root.join("shell/powershell-hook.ps1"),
    };
    AppConfig::default().save(&paths).unwrap();
    std::fs::write(&paths.secrets_file, "{\"api_keys\":{}}\n").unwrap();
    std::fs::set_permissions(&paths.secrets_file, std::fs::Permissions::from_mode(0o644)).unwrap();
    std::fs::set_permissions(
        paths.mcp_config_file(),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    std::fs::write(&paths.config_file, "invalid jsonc").unwrap();

    AppConfig::init_files(&paths).unwrap();

    let mode = std::fs::metadata(&paths.secrets_file)
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600);
    let mcp_mode = std::fs::metadata(paths.mcp_config_file())
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mcp_mode, 0o600);
}
