local investigate = require("investigate")

sai.register_tool({
    name = "linux_input_method_diagnose",
    description = "Run a Linux input method diagnosis sub-agent using runtime evidence, framework detection, display mode checks, and input method path analysis. / 运行 Linux 输入法诊断子代理，基于运行时证据、框架识别、显示模式和输入法路径分析输出诊断报告。",
    parameters = {
        type = "object",
        properties = {
            issue = { type = "string", description = "Input method issue or symptom. / 输入法问题或现象。" },
            target = { type = "string", description = "Optional target app/process name, e.g. steam or qq. / 可选目标应用或进程名，例如 steam 或 qq。" },
        },
        required = sai.json.array({ "issue" }),
        additionalProperties = false,
    },
    access = "read_only",
    execute = investigate.run,
})
