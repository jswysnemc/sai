use super::identity::wide;
use crate::sandbox::SandboxPolicy;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::Authorization::*;
use windows_sys::Win32::Security::{
    ACL, DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID, SUB_CONTAINERS_AND_OBJECTS_INHERIT,
};
use windows_sys::Win32::Storage::FileSystem::{
    DELETE, FILE_GENERIC_EXECUTE, FILE_GENERIC_READ, FILE_GENERIC_WRITE,
};

const READ: u32 = FILE_GENERIC_READ | FILE_GENERIC_EXECUTE;
const WRITE: u32 = FILE_GENERIC_WRITE | DELETE;
const DENY_WRITE: u32 = 0x2 | 0x4 | 0x10 | 0x100 | DELETE | 0x40000 | 0x80000 | 0x40;

/// 【Windows沙箱】【路径授权】只增删本次容器 SID 的权限项，不还原整个 DACL 覆盖其他并发修改。
pub(super) struct Grants {
    sid: PSID,
    paths: Vec<PathBuf>,
    journal: super::journal::Journal,
}
impl Grants {
    /// 【Windows沙箱】【权限应用】保护路径先拒绝，再允许工作区和工具读取。
    /// 参数：容器 SID、解析策略和 Shell 路径；返回自动回收授权的守卫。
    pub(super) fn apply(
        sid: PSID,
        journal: super::journal::Journal,
        policy: &SandboxPolicy,
        shell: &Path,
    ) -> Result<Self> {
        let mut grants = Self {
            sid,
            paths: Vec::new(),
            journal,
        };
        grants.deny_tree(&super::journal::root()?, READ | DENY_WRITE)?;
        for path in &policy.deny_read {
            if path.exists() {
                grants.deny_tree(path, READ | DENY_WRITE)?;
            }
        }
        for path in &policy.write_protected {
            if !path.exists() && path.parent().is_some_and(Path::is_dir) {
                std::fs::create_dir(path)?;
            }
            if path.exists() {
                grants.deny_tree(path, DENY_WRITE)?;
                // 1. 【Windows沙箱】【祖先保护】禁止把保护路径的祖先整体移动后替换路径
                for ancestor in path.ancestors().skip(1).take_while(|parent| {
                    policy
                        .writable_roots
                        .iter()
                        .any(|root| parent.starts_with(root))
                }) {
                    grants.journal.register(ancestor)?;
                    update_exact(ancestor, sid, DELETE, DENY_ACCESS, false)?;
                    if !grants.paths.iter().any(|entry| entry == ancestor) {
                        grants.paths.push(ancestor.into());
                    }
                }
            }
        }
        grants.add(&policy.cwd, READ, GRANT_ACCESS)?;
        for root in &policy.writable_roots {
            grants.add(root, READ | WRITE, GRANT_ACCESS)?;
        }
        // 1. 【Windows沙箱】【工具目录】只授予 PATH 中工具目录读取，不扩展为整个用户目录
        if let Some(path) = std::env::var_os("PATH") {
            for root in
                std::env::split_paths(&path).filter(|root| root.is_dir() && !system_directory(root))
            {
                grants.add(&root, READ, GRANT_ACCESS)?;
            }
        }
        if let Some(parent) = shell.parent().filter(|path| !system_directory(path)) {
            grants.add(parent, READ, GRANT_ACCESS)?;
        }
        Ok(grants)
    }

    /// 【Windows沙箱】【拒绝继承】为现存后代添加显式拒绝，防止子对象显式允许覆盖父目录拒绝。
    /// 参数：保护路径和拒绝权限；返回结果，重解析点拒绝启动。
    fn deny_tree(&mut self, path: &Path, rights: u32) -> Result<()> {
        use std::os::windows::fs::MetadataExt;
        let metadata = std::fs::symlink_metadata(path)?;
        anyhow::ensure!(
            metadata.file_attributes() & 0x400 == 0,
            "protected path is a reparse point: {}",
            path.display()
        );
        self.add(path, rights, DENY_ACCESS)?;
        if metadata.is_dir() {
            for item in std::fs::read_dir(path)? {
                self.deny_tree(&item?.path(), rights)?;
            }
        }
        Ok(())
    }

    /// 【Windows沙箱】【授权写入】添加单个权限项并登记清理路径；参数为路径、访问位和方式，返回结果。
    fn add(&mut self, path: &Path, rights: u32, mode: ACCESS_MODE) -> Result<()> {
        self.journal.register(path)?;
        update(path, self.sid, rights, mode)?;
        if !self.paths.iter().any(|existing| existing == path) {
            self.paths.push(path.to_path_buf());
        }
        Ok(())
    }
}
impl Drop for Grants {
    /// 【Windows沙箱】【授权回收】撤回专属 SID 权限，保留其他主体的全部授权；无参数、无返回值。
    fn drop(&mut self) {
        let mut complete = true;
        for path in self.paths.iter().rev() {
            if let Err(error) = update(path, self.sid, 0, REVOKE_ACCESS) {
                complete = false;
                eprintln!("【Windows沙箱】【授权回收】{}: {error:#}", path.display());
            }
        }
        if complete {
            if let Err(error) = self.journal.complete() {
                eprintln!("【Windows沙箱】【日志清理】{error:#}");
            }
        }
    }
}

/// 【Windows沙箱】【撤回授权】仅移除指定身份权限；参数为路径和 SID，返回撤回结果。
pub(super) fn revoke(path: &Path, sid: PSID) -> Result<()> {
    update(path, sid, 0, REVOKE_ACCESS)
}

/// 【Windows沙箱】【DACL 合并】仅改变指定 SID 的显式 ACE，Windows 负责目录继承。
/// 参数：文件路径、SID、访问位及操作方式；返回权限更新结果。
fn update(path: &Path, sid: PSID, rights: u32, mode: ACCESS_MODE) -> Result<()> {
    update_exact(path, sid, rights, mode, true)
}

/// 【Windows沙箱】【继承范围】可仅限制目录自身，避免把祖先删除限制传播到正常文件。
/// 参数：目标路径、SID、权限、操作与继承标记；返回权限更新结果。
fn update_exact(
    path: &Path,
    sid: PSID,
    rights: u32,
    mode: ACCESS_MODE,
    inherit: bool,
) -> Result<()> {
    // 1. 【Windows沙箱】【并发授权】跨进程串行修改 DACL，避免两个启动器覆盖彼此授权
    let root = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .context("LOCALAPPDATA is unavailable")?
        .join("sai");
    std::fs::create_dir_all(&root)?;
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(root.join("sandbox-acl.lock"))?;
    lock.lock()?;
    let path_wide = wide(&path.to_string_lossy());
    let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
    let mut old_acl: *mut ACL = std::ptr::null_mut();
    let code = unsafe {
        GetNamedSecurityInfoW(
            path_wide.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut old_acl,
            std::ptr::null_mut(),
            &mut descriptor,
        )
    };
    anyhow::ensure!(code == 0, "read DACL {}: {code}", path.display());
    let entry = EXPLICIT_ACCESS_W {
        grfAccessPermissions: rights,
        grfAccessMode: mode,
        grfInheritance: if inherit && path.is_dir() {
            SUB_CONTAINERS_AND_OBJECTS_INHERIT
        } else {
            0
        },
        Trustee: TRUSTEE_W {
            pMultipleTrustee: std::ptr::null_mut(),
            MultipleTrusteeOperation: NO_MULTIPLE_TRUSTEE,
            TrusteeForm: TRUSTEE_IS_SID,
            TrusteeType: TRUSTEE_IS_UNKNOWN,
            ptstrName: sid.cast(),
        },
    };
    let mut new_acl = std::ptr::null_mut();
    let code = unsafe { SetEntriesInAclW(1, &entry, old_acl, &mut new_acl) };
    let result = if code == 0 {
        let code = unsafe {
            SetNamedSecurityInfoW(
                path_wide.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                new_acl,
                std::ptr::null(),
            )
        };
        if code == 0 {
            Ok(())
        } else {
            Err(std::io::Error::from_raw_os_error(code as i32)).context("write sandbox DACL")
        }
    } else {
        Err(std::io::Error::from_raw_os_error(code as i32)).context("merge sandbox DACL")
    };
    unsafe {
        LocalFree(descriptor);
        if !new_acl.is_null() {
            LocalFree(new_acl.cast());
        }
    }
    result
}

/// 【Windows沙箱】【系统目录】系统文件沿用 LPAC 系统 ACL，不修改受保护安装目录。
/// 参数：候选路径；返回是否位于系统安装目录。
fn system_directory(path: &Path) -> bool {
    let value = path
        .to_string_lossy()
        .trim_end_matches('\\')
        .to_ascii_lowercase();
    ["SystemRoot", "ProgramFiles", "ProgramFiles(x86)"]
        .iter()
        .filter_map(std::env::var_os)
        .any(|root| {
            let root = root
                .to_string_lossy()
                .trim_end_matches('\\')
                .to_ascii_lowercase();
            value == root || value.starts_with(&format!("{root}\\"))
        })
}
