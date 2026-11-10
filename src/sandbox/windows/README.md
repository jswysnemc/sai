# Windows 命令与隔离

内部启动器在 `CreateProcessW` 创建子进程时绑定关闭即终止的 Job，随后恢复执行。
标准输入输出使用单独复制的句柄，并通过 HANDLE_LIST 限制继承范围。
隔离命令另外使用独立 AppContainer SID 与 LPAC 属性；环境变量由父进程清理并继承，不写入启动参数。
默认可读范围为工作目录、显式可写根、实际 Shell 安装目录及系统已有 LPAC 权限。PATH 不自动扩大文件授权。
文件授权只增删本次 SID 的 ACE。授权日志和活动锁用于异常退出后的清理。

原生行为验证：

```powershell
cargo run --locked --manifest-path scripts/windows_sandbox_regression/Cargo.toml
```

该命令验证真实 Shell 读取、只读工作区拒绝写入、允许工作区写入、隐藏文件、保护路径和越界拒绝。
Windows 编译流水线执行这组测试；交叉编译不能替代原生行为验证。

API 参考：

- https://learn.microsoft.com/en-us/windows/win32/secauthz/implementing-an-appcontainer
- https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-security_capabilities
- https://github.com/microsoft/SandboxSecurityTools/tree/main/LaunchAppContainer
