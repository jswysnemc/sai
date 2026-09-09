local M = {}

--- 【AUR】【助手选择】按 paru、yay 顺序探测，缺失、失败或超时才尝试下一项
--- @return string|nil 可用的助手名称
function M.helper()
  for _, name in ipairs({ "paru", "yay" }) do
    local ok, output = pcall(sai.process.output, name .. "_version", {}, { timeout_ms = 5000, max_stdout_bytes = 8192, max_stderr_bytes = 8192 })
    if ok and not output.timed_out and output.status == 0 then return name end
  end
end

--- 【AUR】【命令结果】保留原工具的退出码和文本字段，超时不会当作普通退出
--- @param command string 报告名称
--- @param output table 宿主进程结果
--- @param seconds number 当前步骤的超时秒数
--- @return table 统一命令结果
function M.result(command, output, seconds)
  if output.timed_out then error(command .. " timed out after " .. seconds .. "s") end
  return { ok = output.status == 0, command = command, exit_code = output.status,
    stdout = sai.text.trim(output.stdout), stderr = sai.text.trim(output.stderr),
    stdout_truncated = output.stdout_truncated, stderr_truncated = output.stderr_truncated }
end

--- 【AUR】【构建文件获取】助手仅下载构建文件，不执行构建或安装步骤
--- @param workspace userdata 私有目录
--- @param helper string 已探测助手
--- @param package string 已验证包名
--- @return nil 下载失败时抛出错误
function M.fetch(workspace, helper, package)
  local result = M.result(helper, workspace:process(helper .. "_fetch", { package = package }, { timeout_ms = 120000 }), 120)
  if not result.ok then error(helper .. " failed: " .. result.stderr) end
end

return M
