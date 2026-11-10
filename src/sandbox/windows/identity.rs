use anyhow::{Context, Result};
use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::Authorization::{ConvertSidToStringSidW, ConvertStringSidToSidW};
use windows_sys::Win32::Security::Isolation::{
    CreateAppContainerProfile, DeleteAppContainerProfile,
};
use windows_sys::Win32::Security::{FreeSid, PSID, SID_AND_ATTRIBUTES};
use windows_sys::Win32::System::SystemServices::SE_GROUP_ENABLED;

/// 【Windows沙箱】【身份】每次启动使用独立容器 SID，避免并发命令共享授权。
pub(super) struct Identity {
    pub(super) sid: PSID,
    name: Vec<u16>,
}

/// 【Windows沙箱】【宽字符】转换 Win32 字符串参数；参数为文本，返回以零结尾的 UTF-16。
pub(super) fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

impl Identity {
    /// 【Windows沙箱】【身份创建】创建本次容器；参数为唯一名称，返回拥有 SID 的身份。
    pub(super) fn create(name: &str) -> Result<Self> {
        let name = wide(name);
        let mut sid = std::ptr::null_mut();
        let result = unsafe {
            CreateAppContainerProfile(
                name.as_ptr(),
                name.as_ptr(),
                name.as_ptr(),
                std::ptr::null(),
                0,
                &mut sid,
            )
        };
        anyhow::ensure!(
            result >= 0,
            "CreateAppContainerProfile failed: 0x{:08x}",
            result as u32
        );
        Ok(Self { sid, name })
    }

    /// 【Windows沙箱】【身份记录】序列化 SID 供崩溃后清理授权；无参数，返回标准 SID 字符串。
    pub(super) fn sid_string(&self) -> Result<String> {
        let mut text = std::ptr::null_mut();
        anyhow::ensure!(
            unsafe { ConvertSidToStringSidW(self.sid, &mut text) } != 0,
            "cannot serialize sandbox SID"
        );
        let result = unsafe {
            let mut len = 0;
            while *text.add(len) != 0 {
                len += 1;
            }
            let result = String::from_utf16_lossy(std::slice::from_raw_parts(text, len));
            LocalFree(text.cast());
            result
        };
        Ok(result)
    }
}

impl Drop for Identity {
    /// 【Windows沙箱】【身份回收】进程结束后删除容器档案并释放 SID；无参数、无返回值。
    fn drop(&mut self) {
        unsafe {
            DeleteAppContainerProfile(self.name.as_ptr());
            FreeSid(self.sid);
        }
    }
}

/// 【Windows沙箱】【能力集合】仅添加运行 Shell 所需能力，断网模式不添加任何网络能力。
pub(super) struct Capabilities {
    owned: Vec<PSID>,
    pub(super) entries: Vec<SID_AND_ATTRIBUTES>,
}
impl Capabilities {
    /// 【Windows沙箱】【能力解析】构造 LPAC 的注册表只读与 COM 能力；参数为是否允许网络，返回能力列表。
    pub(super) fn new(network: bool) -> Result<Self> {
        let mut this = Self {
            owned: Vec::new(),
            entries: Vec::new(),
        };
        // 1. 【Windows沙箱】【最小能力】微软 LaunchAppContainer 示例的 cmd.exe 基础能力
        for sid in [
            "S-1-15-3-1024-2405443489-874036122-4286035555-1823921565-1746547431-2453885448-3625952902-991631256",
            "S-1-15-3-1024-1065365936-1281604716-3511738428-1654721687-432734479-3232135806-4053264122-3456934681",
        ].into_iter().chain(network.then_some("S-1-15-3-1")) {
            let mut value = std::ptr::null_mut();
            if unsafe { ConvertStringSidToSidW(wide(sid).as_ptr(), &mut value) } == 0 {
                return Err(std::io::Error::last_os_error()).context("parse sandbox capability");
            }
            this.owned.push(value);
            this.entries.push(SID_AND_ATTRIBUTES { Sid: value, Attributes: SE_GROUP_ENABLED as u32 });
        }
        Ok(this)
    }
}
impl Drop for Capabilities {
    /// 【Windows沙箱】【能力回收】释放解析的能力 SID；无参数、无返回值。
    fn drop(&mut self) {
        for sid in &self.owned {
            unsafe {
                LocalFree(*sid);
            }
        }
    }
}
