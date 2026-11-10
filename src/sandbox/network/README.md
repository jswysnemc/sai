# 命令网络授权

```jsonc
{
  "sandbox": {
    "enabled": true,
    "network": "allow",
    "allowed_domains": ["github.com", "*.githubusercontent.com", "static.rust-lang.org"]
  }
}
```

仅在沙箱命令中生效。允许 HTTP 80 和 HTTPS 443；通配规则只匹配子域，不隐式包含根域。
空列表沿用 `network=allow` 的完整网络语义。`network=deny` 始终断网。

Linux 使用独立网络命名空间与受控代理通道，命令不能通过取消代理变量直接联网。
命令子进程额外禁止 Unix/VSOCK 等套接字和 io_uring，防止借用宿主本地服务绕过域名限制。
macOS 使用 Seatbelt，只允许连接本次代理端口，不开放其他网络和 Unix 域套接字。
代理校验域名、端口和解析地址，拒绝私网、回环及链路本地地址，随后固定地址连接。
每次运行最多 64 个转发连接，头部与等待时间均有上限，命令结束后取消全部连接。

Windows 当前拒绝非空域名列表，不会自动退回完整网络。Windows 的断网与普通联网策略由 AppContainer 执行。

原生回归（须先编译）：

```sh
SAI_TEST_BINARY=target/debug/sai python3 scripts/network-sandbox-regression.py
```

该测试断言直接连接被拒绝、未授权域名返回 403，Linux 额外验证 Unix 套接字限制。
代理单元测试不需要外网；允许域名的 HTTPS 访问已在 Linux 原生环境验证。
