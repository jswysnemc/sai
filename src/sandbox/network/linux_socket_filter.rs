use std::io;
use std::os::unix::process::CommandExt;

/// 【网络授权】【套接字隔离】限制实际命令只能创建网络命名空间内的 TCP/IP 套接字。
/// 参数：待启动命令；返回无，子进程安装失败时 spawn 返回错误。
pub(super) fn install(command: &mut tokio::process::Command) {
    unsafe {
        command.as_std_mut().pre_exec(apply);
    }
}

/// 【网络授权】【系统调用过滤】禁止 Unix、VSOCK 等宿主通道与异步绕过接口，拒绝兼容架构切换。
/// 参数：无；返回内核过滤器安装结果。
fn apply() -> io::Result<()> {
    #[cfg(target_arch = "x86_64")]
    const ARCH: u32 = 0xc000003e;
    #[cfg(target_arch = "aarch64")]
    const ARCH: u32 = 0xc00000b7;
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    const ARCH: u32 = 0;
    if ARCH == 0 {
        return Err(io::Error::from_raw_os_error(libc::ENOTSUP));
    }
    let statement = |code, k| libc::sock_filter {
        code,
        jt: 0,
        jf: 0,
        k,
    };
    let jump = |k, jt, jf| libc::sock_filter {
        code: 0x15,
        jt,
        jf,
        k,
    };
    let mut filter = [
        statement(0x20, 4),
        jump(ARCH, 1, 0),
        statement(0x06, 0x80000000),
        statement(0x20, 0),
        libc::sock_filter {
            code: 0x35,
            jt: 0,
            jf: 1,
            k: 0x40000000,
        },
        statement(0x06, 0x00050000 | libc::EPERM as u32),
        jump(libc::SYS_io_uring_setup as u32, 0, 1),
        statement(0x06, 0x00050000 | libc::EPERM as u32),
        jump(libc::SYS_io_uring_enter as u32, 0, 1),
        statement(0x06, 0x00050000 | libc::EPERM as u32),
        jump(libc::SYS_io_uring_register as u32, 0, 1),
        statement(0x06, 0x00050000 | libc::EPERM as u32),
        jump(libc::SYS_ptrace as u32, 0, 1),
        statement(0x06, 0x00050000 | libc::EPERM as u32),
        jump(libc::SYS_process_vm_writev as u32, 0, 1),
        statement(0x06, 0x00050000 | libc::EPERM as u32),
        jump(libc::SYS_process_vm_readv as u32, 0, 1),
        statement(0x06, 0x00050000 | libc::EPERM as u32),
        jump(libc::SYS_pidfd_getfd as u32, 0, 1),
        statement(0x06, 0x00050000 | libc::EPERM as u32),
        jump(libc::SYS_socketpair as u32, 0, 1),
        statement(0x06, 0x00050000 | libc::EPERM as u32),
        jump(libc::SYS_socket as u32, 0, 5),
        statement(0x20, 16),
        jump(libc::AF_INET as u32, 3, 0),
        jump(libc::AF_INET6 as u32, 2, 0),
        statement(0x06, 0x00050000 | libc::EPERM as u32),
        statement(0x06, 0x7fff0000),
        statement(0x06, 0x7fff0000),
    ];
    let program = libc::sock_fprog {
        len: filter.len() as u16,
        filter: filter.as_mut_ptr(),
    };
    unsafe {
        if libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0
            || libc::prctl(libc::PR_SET_SECCOMP, 2, &program) != 0
        {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}
