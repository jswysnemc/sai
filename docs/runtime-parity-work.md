# Agent 运行时对齐记录

参考：Sai `14c4075c`，本地 Codex `627b579219`。

## 已实现

- 子代理用量去重、原始响应签名与图片附件保留、进度实时交付
- 子代理对话检查点、收件箱消费水位、重启恢复、历史留档和上下文压缩
- 模型传输有限重试、断流文本撤销，压缩与子代理共用恢复流程
- 只读工具有界并发、逐项结果持久化、按调用 ID 更新界面
- Responses/Anthropic/Chat 完整调用事件，文件只读前缀在响应结束前执行；响应重试、参数或顺序变化会取消缓存
- PTY 命令会话、write_stdin、会话所有权、有界输出、取消时清理进程树
- 主代理后台预压缩、摘要来源指纹校验、取消与实际用量核算
- `/goal` 运行中控制、目标替换后的计量绑定、预算停止、规划模式续轮限制、外部任务等待
- 用户指导进入当前任务上下文，目标正文只由显式目标操作修改
- 队列默认下一工具间隙发送，编辑可改为整体任务结束后发送
- 侧栏只显示当前会话运行中和未消费完成任务，消费后隐藏，日志保留
- 全屏工具长命令折叠显示省略号，展开保留完整命令
- Windows 原生 AppContainer 路径隔离、创建时绑定 Job、标准句柄继承限制、授权恢复日志
- Linux/macOS 域名代理与操作系统网络限制，配置为 `sandbox.allowed_domains`

## 边界

- 流式提前执行仅覆盖连续文件只读前缀，最多 8 项；写调用等待完整响应并经过正常审批和持久化
- Windows 域名白名单网络模式明确拒绝执行，尚未接入原生受限代理；不会退回完整网络
- PTY 会话由 write_stdin 消费，侧栏支持查看、停止和清理；面板查看不消费模型输出
- Windows 原生隔离行为与三平台发布编译以对应 GitHub Actions 结果为准

## 验证

- 本地整个工作区：71 个测试套件，4718 项通过，19 项忽略，无失败
- 额外回归：并发队列在目标暂停后拒绝启动工具
- Windows 独立模块通过 GNU 目标交叉编译检查
- Linux 原生网络回归：已授权 HTTPS 成功；直接网络、Unix 套接字、未授权域名均被拒绝
- GitHub Actions：Linux/macOS 完整编译、测试与网络回归；Windows 完整编译、测试及独立沙箱/图形协议回归；Docker 镜像构建

复现命令：

```sh
cargo test --locked -j 1
cargo check --manifest-path scripts/windows_sandbox_regression/Cargo.toml --target x86_64-pc-windows-gnu
SAI_TEST_BINARY=target/debug/sai python3 scripts/network-sandbox-regression.py
```

Windows 原生回归：

```powershell
cargo run --locked --manifest-path scripts/windows_sandbox_regression/Cargo.toml
```
