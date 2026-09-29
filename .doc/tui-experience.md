# TUI 配置与输入体验

## 模块结构

```text
src/config_tui/
├── background.rs                  # 后台任务等待界面与立即返回
├── provider_forms/
│   ├── editor.rs                  # 供应商草稿及分区菜单
│   ├── connection.rs              # 地址、协议、身份与启用状态
│   ├── credentials.rs             # 普通供应商和专用接入共用密钥表单
│   ├── keys.rs                    # 多密钥解析与稳定标识
│   ├── advanced.rs                # 请求高级选项及事务校验
│   └── values.rs                  # 地址和 JSON 规范化
├── provider_fetch.rs              # 共享模型目录服务适配
├── providers/catalog.rs           # 请求去重、并发限制与缓存
├── model_endpoints/
│   ├── mod.rs                     # Jev、生图接入列表与分类菜单
│   ├── editor.rs                  # 接入草稿、模型与协议编辑
│   ├── operations.rs              # 保存、选择默认项及删除引用
│   └── probe.rs                   # 目录导入及实际连接测试
└── compaction/input_edit.rs       # 上下文策略数值输入
src/cli/repl_runtime/composer_frame/
└── input_viewport.rs              # 根据光标裁剪视觉行
src/web/services/model_endpoint_models.rs
                                  # 网页和终端共用生图目录请求
scripts/tui_regression/
├── test_configuration.py          # 慢接口导航、长文本、上下文别名
├── test_endpoint_configuration.py # 接入保存重载、取消与密钥保留
└── mock_endpoints.py              # 本地 Jev 与生图响应
```

## 行为

- 配置主菜单第 2 项管理普通供应商；编辑界面分为连接、密钥、高级请求和连接测试。新增供应商分配独立标识，拒绝覆盖已有标识；已有供应商标识保持固定。
- 配置主菜单第 7 项管理 Jev 与生图模型，第 8 项保存退出。接入列表支持 `a` 新增、`Enter` 编辑、`Space` 设为默认、`d` 删除。Jev 路由与审核沿用原配置。
- 生图支持 `auto`、`openai-images`、`gemini` 协议、完整请求地址、手动模型及目录选择；Jev 支持多接入和完整密钥池。
- 各分区先修改草稿，退出编辑器可丢弃所有分区修改；写入磁盘仍通过配置主菜单统一确认。编辑名称或地址不会清空密钥池。
- `/context edit` 和兼容别名 `/content edit` 打开上下文策略面板。数值进入编辑时全选；支持左右移动、Home/End、Delete/Backspace、Ctrl+A 全选、Ctrl+U 清空、Tab 确认并移动、Esc 撤销输入。
- 长文本输入先折成视觉行，再显示光标附近内容。上下键逐行查看，Ctrl+A/E 跳到首尾；窗口缩小时优先保留光标所在行。原有显式粘贴原子块机制保持不变。

## 缺陷原因与异步边界

供应商模型请求原本就在工作线程中。旧主循环只比较轮询请求前后的状态，没有比较按键操作与上一帧的状态，导致按键已改变焦点而画面停在旧栏。现在按上一帧签名决定重绘，子表单关闭后恢复列表。

目录请求按配置指纹去重，最多四个并发请求。切换时立即显示本地模型，旧请求结果只进入对应缓存。失败保留本地目录，按 `r` 显式重试。自动目录请求不再串行等待多个外部公共目录；仍保留供应商响应携带的上下文、输出上限等元数据。

连接测试运行于工作线程，`Esc` 或 `q` 立即退出等待界面。已经发出的网络请求由自身超时结束，返回结果不会继续修改已退出的草稿。

## 验证

- 修复前已通过真实伪终端复现：慢模型接口期间右键不能更新画面；长输入移动到开头仍只能看到折叠尾部。
- `cargo test --workspace --quiet`：工作区测试通过；后续最终主程序测试为 3307 项通过、17 项原有忽略。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo build --bin sai --quiet`：通过。
- `uv run scripts/tui_regression.py`：覆盖配置、输入、流式回复、窗口缩放、工具进度与补全；全部 20 个用例通过。
- 网络验证使用本地 HTTP 服务和虚构密钥，实际经过 TUI、目录请求、Jev 判断及生图响应解析。检查选中密钥、请求路径、生成模型、目录保存和配置重载。
- 本次修改涉及的代码文件均小于 750 行，最大文件为 `providers.rs`，660 行。
