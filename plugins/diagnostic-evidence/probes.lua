local text = require("text")
local reports = require("report")
local settings = require("settings")
local M = {}
local Probe = {}
Probe.__index = Probe

--- 【诊断采集】【调用实例】所有缓存只属于本次采集，避免跨会话保存环境与文件信息
--- @param report table 待填充报告
--- @return table 受限宿主能力封装
function M.new(report)
    local ok, home = pcall(sai.env.get, "HOME")
    return setmetatable({report=report, home=ok and home or nil, environment={}, missing={}}, Probe)
end

--- 【诊断采集】【缺失证据】去重并限制缺失信息数量，明确权限不足与输出截断
--- @param message string 缺失原因
--- @return nil
function Probe:missing_evidence(message)
    message = text.clip(text.redact(message, self.home), 500)
    if not self.missing[message] and #self.report.missing_evidence < 64 then
        self.missing[message] = true
        self.report.missing_evidence[#self.report.missing_evidence + 1] = message
    end
end

--- 【诊断采集】【环境变量】逐项访问授权变量，缺失值和拒绝访问分别处理
--- @param name string 精确变量名
--- @return string|nil 环境值
function Probe:env(name)
    if self.environment[name] ~= nil then return self.environment[name] or nil end
    local ok, value = pcall(sai.env.get, name)
    if not ok then self:missing_evidence("environment " .. name .. ": " .. tostring(value)); value = nil end
    self.environment[name] = value or false
    return value
end

--- 【诊断采集】【文本读取】只读取授权路径中的有界 UTF-8 文本，二进制字段采用替换解码
--- @param path string 路径
--- @param maximum integer|nil 字节上限
--- @return string|nil 正文
function Probe:read(path, maximum)
    local ok, output = pcall(sai.fs.read_text, path, {max_bytes=maximum or 524288, lossy=true})
    if not ok then self:missing_evidence("file " .. path .. ": " .. tostring(output)); return nil end
    if output.truncated then self:missing_evidence("file " .. path .. ": text truncated at byte limit") end
    return output.text
end

--- 【诊断采集】【目录枚举】最多读取指定条数，截断不能当作完整扫描
--- @param path string 目录
--- @param maximum integer|nil 条数上限
--- @return table 条目数组
function Probe:directory(path, maximum)
    local ok, output = pcall(sai.fs.read_dir, path, {max_entries=maximum or 300})
    if not ok then self:missing_evidence("directory " .. path .. ": " .. tostring(output)); return sai.json.array() end
    if output.truncated then self:missing_evidence("directory " .. path .. ": entries truncated at limit") end
    return output.entries
end

--- 【诊断采集】【存在检查】区分不存在和授权或系统错误
--- @param path string 路径
--- @return boolean 是否存在
function Probe:exists(path)
    local ok, output = pcall(sai.fs.stat, path)
    if not ok then self:missing_evidence("metadata " .. path .. ": " .. tostring(output)); return false end
    return output ~= nil
end

--- 【诊断采集】【固定进程】按完整授权模板执行，超时、取消与输出限制由宿主管理
--- @param name string 模板名
--- @param parameters table|nil 模板参数
--- @param seconds integer 探测默认秒数
--- @return table 兼容原版的进程证据
function Probe:command(name, parameters, seconds)
    local ok, output = pcall(sai.process.output, name, parameters or {}, {
        timeout_ms=math.min(seconds * 1000, settings.command_timeout_ms),
        max_stdout_bytes=math.max(1, settings.max_stdout_chars * 4),
        max_stderr_bytes=math.max(1, settings.max_stderr_chars * 4),
    })
    if not ok then
        self:missing_evidence("process " .. name .. ": " .. tostring(output))
        return {status=sai.json.null, stdout="", stderr=text.clip(text.redact(tostring(output), self.home), settings.max_stderr_chars), timed_out=false}
    end
    if output.stdout_truncated or output.stderr_truncated then self:missing_evidence("process " .. name .. ": output truncated at byte limit") end
    if output.timed_out then self:missing_evidence("process " .. name .. ": timed out") end
    output.stdout = text.clip(output.stdout, settings.max_stdout_chars)
    output.stderr = text.clip(output.stderr, settings.max_stderr_chars)
    return output
end

--- 【诊断采集】【命令路径】只查询名称，不执行目标程序
--- @param name string 命令名称
--- @return string|nil 找到的路径
function Probe:command_path(name)
    if not text.executable_name(name) then return nil end
    local output = self:command("command-path", {target=name}, 2)
    local first = text.lines(output.stdout)[1]
    if output.status == 0 and first and sai.text.trim(first) ~= "" then return sai.text.trim(first) end
end

--- 【诊断采集】【命令检查】记录命令可用性，返回路径供后续采集复用
--- @param name string 命令名称
--- @return string|nil 找到的路径
function Probe:command_exists(name)
    local path = self:command_path(name)
    reports.check(self.report, "command." .. name .. ".exists", path and "ok" or "unknown",
        name .. (path and " is available" or " is not available"), sai.json.array(path and {path} or {}))
    return path
end

--- 【诊断采集】【环境事实】忽略空值，按原字段名称记录脱敏环境
--- @param key string 报告字段
--- @param name string 环境变量
--- @return nil
function Probe:fact_env(key, name)
    local value = self:env(name)
    if value and sai.text.trim(value) ~= "" then self.report.facts[key] = text.redact(value, self.home) end
end

--- 【诊断采集】【近期日志】快速模式不读日志，其余模式只保留匹配关键字的八十行
--- @param args table 规范参数
--- @param needles table 关键字
--- @return nil
function Probe:recent_logs(args, needles)
    if args.depth == "quick" or not self:command_path("journalctl") then return end
    local output = self:command("recent-logs", {since="-" .. args.recent_minutes .. "min"}, 5)
    local matches, lower = {}, {}
    for _, needle in ipairs(needles) do lower[#lower + 1] = needle:lower() end
    for _, line in ipairs(text.lines(output.stdout)) do
        if text.contains_any(line:lower(), lower) then matches[#matches + 1] = line end
        if #matches == 80 then break end
    end
    reports.log(self.report, "journalctl --user recent filtered", table.concat(matches, "\n"))
end

return M
