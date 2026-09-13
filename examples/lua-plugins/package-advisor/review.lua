local metadata = require("metadata")
local files = require("files")
local processes = require("processes")
local state = require("state")
local M = {}

--- 【AUR】【审查流程】获取构建文件、计算风险并记录后续确认依据，不执行构建脚本
--- @param args table 工具参数
--- @param ctx table 可信会话上下文
--- @return table 与原工具兼容的审查报告
function M.run(args, ctx)
  local package = metadata.package(args)
  state.begin(package)
  local info = metadata.fetch(package)
  local workspace = sai.workspace.open("review/" .. package)
  local helper, root = processes.helper(), "."
  if helper then processes.fetch(workspace, helper, package)
  else root = metadata.snapshot(workspace, info) end
  local build_dir = files.build_dir(workspace, root)
  local reviewed, complete = files.collect(workspace, build_dir)
  local risk = require("risk").evaluate(reviewed)
  local allowed = complete and risk.level ~= "high"
  local paths = sai.json.array()
  for _, file in ipairs(reviewed) do paths[#paths + 1] = file.path end
  local result = { ok = true, package = package, build_dir = workspace:path() .. (build_dir == "." and "" or "/" .. build_dir),
    fetched_by = helper or "curl-fallback", aur_metadata = info, risk = risk, install_allowed = allowed,
    review_complete = complete, files_reviewed = paths, files = reviewed, review_rules = require("rules"),
    output_instruction = "Use review_rules exactly, but omit the PAC_DECISION machine-readable line in the final answer. Mention risk.level, review_complete and install_allowed. Do not install, build or run makepkg. Incomplete review evidence blocks installation. If install_allowed is true, ask the user whether to install and stop." }
  -- 【AUR】【结果预算】1. 确认报告可以完整交付之后才记录安装依据
  sai.json.encode(result)
  state.record(package, risk, allowed, ctx)
  return result
end

return M
