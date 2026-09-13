local M = {}

--- 【AUR】【撤销旧记录】新审查开始前清除旧许可，失败不能留下旧安装依据
--- @param package string 已验证包名
--- @return nil 无返回值
function M.begin(package) sai.storage.set("review/" .. package, nil) end

--- 【AUR】【记录审查】将风险、完整性与本次操作标识保存到插件会话状态
--- @param package string 包名
--- @param risk table 风险结果
--- @param allowed boolean 是否允许后续确认
--- @param ctx table 可信调用上下文
--- @return nil 无返回值
function M.record(package, risk, allowed, ctx)
  sai.storage.set("review/" .. package, { package = package, reviewed_at_unix = sai.time.now(),
    risk = risk, install_allowed = allowed, user_confirmed_install = false, operation_id = ctx.operation_id })
end

--- 【AUR】【消费确认】必须在另一操作中确认，只允许一个安装调用原子消费审查记录
--- @param package string 包名
--- @param ctx table 可信调用上下文
--- @return table 已消费的审查记录
function M.confirm(package, ctx)
  local key = "review/" .. package
  local review = sai.storage.get(key)
  if review == nil or review == sai.json.null then error("AUR package must be reviewed before install: " .. package) end
  if not review.install_allowed then error("AUR package review did not allow install: " .. package) end
  if review.operation_id == ctx.operation_id then
    error("install_aur_package cannot run in the same turn as review_aur_package. Ask the user to confirm installation in a new turn first.")
  end
  if review.installation_started then error("AUR review has already been consumed; review again before another install: " .. package) end
  local confirmed = sai.json.decode(sai.json.encode(review))
  confirmed.user_confirmed_install = true
  confirmed.user_confirmed_at_unix = sai.time.now()
  confirmed.installation_started = true
  if not sai.storage.compare_exchange(key, review, confirmed) then error("AUR review changed; review again before installing: " .. package) end
  return confirmed
end

return M
