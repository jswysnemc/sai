local M = {}
local patterns = { "curl ", "wget ", "| sh", "|sh", "chmod 777", "chown root", "setcap ",
  "systemctl enable", "rm -rf /", "skipsums", "sha256sums=('skip'", 'sha256sums=("skip"' }

--- 【AUR】【风险规则】保留原有模式顺序及高、中、低三级判断
--- @param files table 审查文件数组
--- @return table 风险级别及具体命中
function M.evaluate(files)
  local findings, high = sai.json.array(), false
  for _, file in ipairs(files) do
    local lower = string.lower(file.content)
    for _, pattern in ipairs(patterns) do
      if lower:find(pattern, 1, true) then
        findings[#findings + 1] = { file = file.path, pattern = pattern }
        high = high or pattern == "| sh" or pattern == "|sh" or pattern == "rm -rf /"
      end
    end
  end
  return { level = high and "high" or (#findings == 0 and "low" or "medium"), findings = findings }
end

return M
