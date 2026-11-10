# Windows 终端图形回归

此测试包直接引用项目中的生产探测模块，避免为图形协议测试构建应用的全部依赖。
每个原生场景在独立子进程和新控制台中执行，不连接用户终端。

Windows 上执行：

```powershell
cargo test --locked --manifest-path scripts/windows_graphics_regression/Cargo.toml -- --nocapture
```

测试覆盖 Ely 环境启动、零虚拟键码应答、Alt 编码应答、逐字符应答、错误和错误编号、超时、
残缺响应、Sixel 能力位、写入失败、输出重定向，以及中文、UTF-16 代理对、重复和释放按键的原始记录保留。
请求发送期间验证 VT 输出模式，结束后验证输入和输出标志准确恢复。

Linux 上可使用 `cargo-xwin` 编译为 `x86_64-pc-windows-msvc`，再通过 Wine 和隔离的 Xvfb
运行生成的测试程序。Wine 测试验证 Win32 调用和记录处理；Ely 实际窗口显示仍需在 Windows 上验证。
