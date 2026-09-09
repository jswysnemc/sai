local metadata = require("metadata")
local files = require("files")
local processes = require("processes")
local M = {}

--- 【AUR】【安装回退】下载快照，先构建再选择包文件交给 pacman，失败立即停止
--- @param workspace userdata 私有工作目录
--- @param package string 已验证包名
--- @return table 最终步骤的命令结果
local function fallback(workspace, package)
  local root = metadata.snapshot(workspace, metadata.fetch(package))
  local build_dir = files.build_dir(workspace, root)
  local built = processes.result("makepkg", workspace:process("makepkg", {}, { directory = build_dir, timeout_ms = 1800000 }), 1800)
  if not built.ok then return built end
  local archive = files.built_package(workspace, build_dir)
  return processes.result("pacman -U", workspace:process("pacman_install", { archive = archive }, { directory = build_dir, timeout_ms = 900000 }), 900)
end

--- 【AUR】【安装流程】验证平台、显式确认和跨操作审查记录，再执行唯一一次安装尝试
--- @param args table 工具参数
--- @param ctx table 可信会话上下文
--- @return table 安装状态与已消费的审查记录
function M.run(args, ctx)
  local package = metadata.package(args)
  if args.user_confirmed ~= true then error("AUR install requires explicit user confirmation after review: " .. package) end
  if not ctx.allow_writes then error("read-only plugin callback cannot install an AUR package") end
  if sai.system.platform ~= "linux" then
    error("AUR installation is only supported on Linux/Arch Linux; this platform can review AUR build files but cannot run paru, yay, makepkg, or pacman -U")
  end
  local review = require("state").confirm(package, ctx)
  local workspace = sai.workspace.open("install/" .. package)
  local helper, result = processes.helper(), nil
  if helper then
    result = processes.result(helper, workspace:process(helper .. "_install", { package = package }, { timeout_ms = 900000 }), 900)
  else result = fallback(workspace, package) end
  return { ok = result.ok, package = package, review = review, install_result = result,
    output_instruction = "Explain that install was allowed because review_aur_package recorded an allowed review state and the user explicitly confirmed installation. Include install success or failure concisely." }
end

return M
