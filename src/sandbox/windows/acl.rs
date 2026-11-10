use super::identity::wide;
use crate::sandbox::SandboxPolicy;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use windows_sys::Win32::Foundation::{CloseHandle, LocalFree, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Security::Authorization::*;
use windows_sys::Win32::Security::{
    EqualSid, GetAce, ACCESS_ALLOWED_ACE, ACE_HEADER, ACL, DACL_SECURITY_INFORMATION,
    PSECURITY_DESCRIPTOR, PSID, SUB_CONTAINERS_AND_OBJECTS_INHERIT,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, DELETE, FILE_FLAG_BACKUP_SEMANTICS, FILE_GENERIC_EXECUTE, FILE_GENERIC_READ,
    FILE_GENERIC_WRITE, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};

const READ: u32 = FILE_GENERIC_READ | FILE_GENERIC_EXECUTE;
const WRITE: u32 = FILE_GENERIC_WRITE | DELETE;

/// 【Windows沙箱】【路径授权】只增删本次容器 SID 的权限项，不还原整个 DACL 覆盖其他并发修改。
pub(super) struct Grants {
    sid: PSID,
    paths: Vec<PathBuf>,
    journal: super::journal::Journal,
}
impl Grants {
    /// 【Windows沙箱】【权限应用】按允许清单授权工作区，保护路径从允许权限中扣除。
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
        // 1. 【Windows沙箱】【允许清单】AppContainer 不依赖拒绝 ACE；保护路径必须从允许授权中排除
        let mut hidden = policy.deny_read.clone();
        hidden.push(super::journal::root()?);
        let protected = &policy.write_protected;
        // 1. 【Windows沙箱】【目录遍历】PowerShell 解析路径需要读取祖先本身，不授予后代继承权限
        for ancestor in policy.cwd.ancestors().skip(1) {
            grants.journal.register(ancestor)?;
            update_exact(ancestor, sid, READ, GRANT_ACCESS, false)?;
            grants.paths.push(ancestor.into());
        }
        grants.allow_tree(&policy.cwd, READ, &hidden, &[])?;
        for root in &policy.writable_roots {
            grants.allow_tree(root, READ | WRITE, &hidden, &protected)?;
        }
        if let Some(parent) = shell.parent().filter(|path| !system_directory(path)) {
            grants.allow_tree(parent, READ, &hidden, &[])?;
        }
        Ok(grants)
    }

    /// 【Windows沙箱】【分段授权】遇到受保护后代时只授权目录本身，分别处理子路径。
    /// 参数：路径、请求权限、隐藏路径和只读路径；返回授权结果。
    fn allow_tree(
        &mut self,
        path: &Path,
        mut rights: u32,
        hidden: &[PathBuf],
        protected: &[PathBuf],
    ) -> Result<()> {
        use std::os::windows::fs::MetadataExt;
        let original_rights = rights;
        if hidden.iter().any(|blocked| under(path, blocked)) {
            return Ok(());
        }
        let metadata = std::fs::symlink_metadata(path)?;
        anyhow::ensure!(
            metadata.file_attributes() & 0x400 == 0,
            "sandbox grant path is a reparse point: {}",
            path.display()
        );
        if protected.iter().any(|readonly| under(path, readonly)) {
            rights &= READ;
        }
        let split = hidden
            .iter()
            .chain(protected)
            .any(|blocked| !same_path(blocked, path) && under(blocked, path));
        if split {
            rights &= !DELETE;
            // 1. 【Windows沙箱】【缺失子路径】禁止在最近存在的父目录创建子目录，仍允许普通文件写入
            if protected.iter().any(|child| {
                !child.exists()
                    && child
                        .ancestors()
                        .skip(1)
                        .find(|parent| parent.exists())
                        .is_some_and(|parent| same_path(parent, path))
            }) {
                rights &= !0x4;
            }
        }
        self.journal.register(path)?;
        update_exact(path, self.sid, rights, GRANT_ACCESS, !split)?;
        if !self.paths.iter().any(|existing| existing == path) {
            self.paths.push(path.into());
        }
        if split && metadata.is_dir() {
            for entry in std::fs::read_dir(path)? {
                self.allow_tree(&entry?.path(), original_rights, hidden, protected)?;
            }
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
    let propagate = inherit && (mode != REVOKE_ACCESS || has_inheritable_sid(old_acl, sid));
    let mut new_acl = std::ptr::null_mut();
    let code = unsafe { SetEntriesInAclW(1, &entry, old_acl, &mut new_acl) };
    let result = if code == 0 {
        let code = if propagate {
            unsafe {
                SetNamedSecurityInfoW(
                    path_wide.as_ptr(),
                    SE_FILE_OBJECT,
                    DACL_SECURITY_INFORMATION,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    new_acl,
                    std::ptr::null(),
                )
            }
        } else {
            // 2. 【Windows沙箱】【仅改本体】仅请求 WRITE_DAC 并直接写入本对象，避免申请额外访问触发共享冲突
            let handle = unsafe {
                CreateFileW(
                    path_wide.as_ptr(),
                    0x00040000,
                    FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                    std::ptr::null(),
                    OPEN_EXISTING,
                    FILE_FLAG_BACKUP_SEMANTICS,
                    std::ptr::null_mut(),
                )
            };
            if handle == INVALID_HANDLE_VALUE {
                std::io::Error::last_os_error().raw_os_error().unwrap_or(5) as u32
            } else {
                let code = unsafe {
                    let mut descriptor: windows_sys::Win32::Security::SECURITY_DESCRIPTOR =
                        std::mem::zeroed();
                    let pointer = (&mut descriptor
                        as *mut windows_sys::Win32::Security::SECURITY_DESCRIPTOR)
                        .cast();
                    if windows_sys::Win32::Security::InitializeSecurityDescriptor(pointer, 1) == 0
                        || windows_sys::Win32::Security::SetSecurityDescriptorDacl(
                            pointer, 1, new_acl, 0,
                        ) == 0
                        || windows_sys::Win32::Security::SetKernelObjectSecurity(
                            handle,
                            DACL_SECURITY_INFORMATION,
                            pointer,
                        ) == 0
                    {
                        std::io::Error::last_os_error().raw_os_error().unwrap_or(5) as u32
                    } else {
                        0
                    }
                };
                unsafe {
                    CloseHandle(handle);
                }
                code
            }
        };
        if code == 0 {
            Ok(())
        } else {
            Err(std::io::Error::from_raw_os_error(code as i32))
                .with_context(|| format!("write sandbox DACL {}", path.display()))
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
        .trim_start_matches(r"\\?\")
        .trim_end_matches('\\')
        .to_ascii_lowercase();
    ["SystemRoot"]
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

/// 【Windows沙箱】【路径归一】按 Windows 大小写规则比较授权范围；参数为路径，返回统一文本。
fn normalized(path: &Path) -> String {
    path.to_string_lossy()
        .trim_start_matches(r"\\?\")
        .trim_end_matches(['\\', '/'])
        .replace('/', "\\")
        .to_lowercase()
}

/// 【Windows沙箱】【范围比较】按目录分隔符判断祖先，避免大小写与前缀误判；参数为路径及根，返回包含关系。
fn under(path: &Path, root: &Path) -> bool {
    let path = normalized(path);
    let root = normalized(root);
    path == root || path.starts_with(&format!("{root}\\"))
}

/// 【Windows沙箱】【路径相等】比较规范化路径；参数为两条路径，返回是否相同。
fn same_path(first: &Path, second: &Path) -> bool {
    normalized(first) == normalized(second)
}

/// 【Windows沙箱】【继承识别】撤回前识别本次 SID 是否拥有可继承 ACE，避免无关目录树遍历。
/// 参数：DACL 与目标 SID；返回是否需要传播撤回。
fn has_inheritable_sid(acl: *mut ACL, sid: PSID) -> bool {
    if acl.is_null() {
        return false;
    }
    unsafe {
        for index in 0..(*acl).AceCount as u32 {
            let mut entry = std::ptr::null_mut();
            if GetAce(acl, index, &mut entry) == 0 {
                return true;
            }
            let header = &*(entry as *const ACE_HEADER);
            if header.AceType <= 1 && header.AceFlags & 3 != 0 {
                let ace = &*(entry as *const ACCESS_ALLOWED_ACE);
                if EqualSid((&ace.SidStart as *const u32).cast_mut().cast(), sid) != 0 {
                    return true;
                }
            }
        }
    }
    false
}
