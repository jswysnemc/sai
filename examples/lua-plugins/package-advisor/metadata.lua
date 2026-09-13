local M = {}

--- 【AUR】【包名校验】包名作为一个 argv 参数，不允许选项前缀、父目录或路径分隔符
--- @param args table 工具参数
--- @return string 规范包名
function M.package(args)
  local package = sai.text.trim(args.package or "")
  if package == "" then error("missing required argument: package") end
  if #package > 128 or not package:match("^[A-Za-z0-9_+][A-Za-z0-9_+%.%-]*$") then
    error("invalid package name: " .. package)
  end
  return package
end

--- 【AUR】【元数据】读取单个包的官方 RPC 记录，服务异常不当作无结果
--- @param package string 已验证包名
--- @return table 包元数据
function M.fetch(package)
  local response = sai.http.request({ url = "https://aur.archlinux.org/rpc/v5/info?arg[]=" .. sai.text.url_encode(package),
    timeout_ms = 20000, max_bytes = 1024 * 1024 })
  if response.status < 200 or response.status >= 300 then error("AUR metadata HTTP " .. response.status) end
  local value = sai.json.decode(response.text)
  if type(value.resultcount) ~= "number" or value.resultcount < 1 or type(value.results) ~= "table" or type(value.results[1]) ~= "table" then
    error("AUR package not found: " .. package)
  end
  return value.results[1]
end

--- 【AUR】【快照获取】仅使用官方来源内的绝对 URLPath，在新目录中展开归档
--- @param workspace userdata 私有目录句柄
--- @param metadata table RPC 元数据
--- @return string 快照相对根目录
function M.snapshot(workspace, metadata)
  local path = metadata.URLPath
  if type(path) ~= "string" or path:sub(1, 1) ~= "/" or path:sub(1, 2) == "//" or path:find("\\", 1, true) then
    error("AUR metadata missing or invalid URLPath")
  end
  workspace:extract_tar_gz({ url = "https://aur.archlinux.org" .. path, destination = "snapshot", timeout_ms = 30000 })
  return "snapshot"
end

return M
