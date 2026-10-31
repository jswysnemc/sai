# Code-Mode（代码行动模式）架构设计与实现方案

本文档梳理 Pi Agent 最新的 Code-Mode 核心架构原理与实现机制，并提供在 Sai 中落地实现该模式的架构规划与工程方案。

参考源码已完整克隆至本地：`/home/snemc/workspace/tmp/sandbox/pi`。
核心包参考：
- `pi/packages/codemode`：通用轻量级 QuickJS WebAssembly 安全沙箱。
- `pi/packages/coding-agent/src/extensions/codemode`：Agent 扩展与工具接入层。
- `pi/packages/coding-agent/docs/codemode.md`：官方使用规范。

---

## 一、 为什么引入 Code-Mode 架构

在传统 Agent 的 Tool Calling（函数调用）体系中，模型每次调用工具均需经历：
`模型输出参数 -> 宿主执行工具 -> 完整原始输出灌入模型上下文 -> 下一轮模型推理`。

该机制在复杂软件工程场景中存在三项瓶颈：
1. **上下文膨胀与污染**：大规模搜索、数据库查询、日志过滤或大文件读取常常产生数万字符的冗余输出。全部灌入上下文会导致 Token 消耗急剧增加，降低模型的注意力精度。
2. **多轮网络往返延迟**：当需要批量处理任务（例如先列出 20 个目标文件，再对每个文件执行检查）时，传统方式必须往返调用 20 轮以上模型，消耗大量网络延迟和计算时间。
3. **缺乏本地逻辑流与控制结构**：传统方式无法在执行端进行循环、批量处理、条件分支判断与多工具结果聚合。

**Code-Mode 解决方案**：
将“单一工具直接调用”收敛为“代码执行（Code-as-Action）”。
模型在单次调用中输出一段 JavaScript（或脚本语言）代码，由安全沙箱在本地执行。沙箱中挂载 `tools` 命名空间代理所有 Agent 工具。模型可在脚本内进行循环、并行（`Promise.all`）、结果过滤，最终仅通过 `text()`、`return` 或 `image()` 将提炼后的关键结论写回上下文。

---

## 二、 Pi 最新 Code-Mode 架构的核心设计要素

### 1. 安全隔离与 WebAssembly 虚拟机
- **引擎选择**：采用编译至 WebAssembly 的 QuickJS 解释器，避免使用存在系统逃逸风险或启动开销沉重的大型运行时。
- **特权剥离**：VM 完全去除了操作系统原生能力、文件系统访问、网络套接字（`fetch`）以及原生定时器。沙箱内唯一与外部连接的通道是由宿主注入的 Host Bridge。
- **前置防御（Prelude Lockdown）**：在执行用户脚本前，通过前置脚本深度遍历并冻结（`Object.freeze`）所有内置原型链对象（`Array.prototype`、`Object.prototype` 等），防止脚本通过原型链污染攻击篡改沙箱通信协议。

### 2. 线程隔离与主动中断机制
- **Worker 线程隔离**：每次执行在独立线程中启动，避免恶意或失控代码（如同步死循环 `while(true) {}`）阻塞宿主事件循环。
- **原子中断信号**：宿主与执行线程之间共享 `SharedArrayBuffer` 中断标志位。QuickJS 在解释执行操作码以及调度微任务队列时定期检查该标志位。一旦超时或收到 Abort 信号，立即强制终止执行。

### 3. 工具暴露与类型声明生成
- **动态 TypeScript 声明渲染**：宿主解析当前已激活工具的 JSON Schema，将其动态转为紧凑的 TypeScript 接口声明（如 `declare const tools: { read(args: { path: string }): Promise<string>; }`）并嵌入系统提示词中。
- **动态 Catalog 与目录预算控制**：当系统注册工具数量过多时，基于字符预算分批渲染。未能在首屏展示的工具，允许模型在沙箱内通过 `searchTools(query)`、`describeTool(name)` 进行动态按需检索。

### 4. 嵌套调用代理与权限复用
- 沙箱内脚本调用 `await tools.<name>(args)` 时，调用请求通过跨线程协议转发给宿主。
- 宿主直接复用主会话的权限检查、只读/写判定以及审计日志链路。
- 嵌套调用的进度与状态实时推送到终端界面展示，但其大体积返回值仅留在沙箱内存中供脚本使用，不直接追加到 LLM 上下文中。

### 5. 跨调用状态保持（Store / Load）
- 提供轻量级同步键值存储 `store(key, value)` 和 `load(key)`。
- 单次脚本修改的状态以增量日志形式（`storeWrites`）记录，写入会话树的分支节点。会话分支回溯或恢复时，自动同步对应分支的持久化状态。

### 6. 输出截断与大文本临时落盘
- 脚本输出超出 Token 上限时，保留首尾片段，并在中间插入省略标记。完整输出自动写入临时文件供模型后续按需按行读取。

---

## 三、 Sai 中的落地实现规划

在 Sai 中实现 Code-Mode 时，建议采取分阶段、模块化的实现方式：

### 1. 模块拆分规划
建议采用如下目录结构：
```text
crates/sai-codemode/             # Code-Mode 核心执行沙箱库（可独立测试）
├── Cargo.toml
└── src/
    ├── lib.rs                   # 公开接口与 Sandbox 定义
    ├── runtime/                 # 解释器驱动与 WASI/沙箱隔离层
    ├── declarations.rs          # ToolSpec 转换为 TypeScript 声明
    ├── store.rs                 # 键值状态持久化
    └── truncate.rs              # 输出安全截断与临时文件落盘

src/tools/codemode.rs            # Sai 内部的 Tool 实现（实现 Tool trait）
src/config/codemode.rs           # 配置项定义（codemode.mode、inline_budget 等）
```

### 2. 执行引擎选型
可二选一方案：
- **方案 A（与 Pi 完全对齐）**：基于 `rquickjs` 或 QuickJS WebAssembly（轻量、确定性高、内存开销极小，约数百 KB 级内存占用）。
- **方案 B（复用现有生态）**：利用 `crates/sai-plugin-runtime` 已有的 Lua 5.4 异步宿主，或引入轻量 JavaScript 引擎。鉴于模型对 JavaScript 语言在工具编排上的成熟度最高，推荐优先支持 JavaScript 语法。

### 3. 配置策略
在 Sai 配置系统增加 `codemode` 节点：
- `codemode.mode`:
  - `"on"`（默认）：常规工具保留声明，同时激活 `codemode` 工具，工具描述中指引可通过脚本调用。
  - `"only"`：收敛模式。对模型隐藏所有常规直接工具，仅暴露 `codemode`，强制模型以编写代码方式调用工具。
- `codemode.inline_budget`: 声明生成的 Token 预算上限。
- `codemode.timeout_ms`: 默认硬超时限制（例如 60000 毫秒）。
