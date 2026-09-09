local M = {}
local MAX_CHARS, MAX_FILES = 24000, 80
local extensions = { install = true, patch = true, diff = true, sh = true, service = true,
  timer = true, socket = true, desktop = true }

--- 【AUR】【相对目录】组合已验证的相对路径，根目录不引入多余的点号段
--- @param root string 相对目录
--- @param path string 子路径
--- @return string 相对路径
function M.join(root, path) return root == "." and path or root .. "/" .. path end

--- 【AUR】【文件存在】区分缺失值和宿主返回的文件属性
--- @param workspace userdata 私有目录
--- @param path string 相对路径
--- @return boolean 是否为普通文件
function M.is_file(workspace, path)
  local info = workspace:stat(path)
  return info ~= nil and info ~= sai.json.null and info.is_file == true
end

--- 【AUR】【构建根目录】检查根目录及一层子目录，按名称排序避免选择不稳定
--- @param workspace userdata 私有目录
--- @param root string 搜索根目录
--- @return string 含 PKGBUILD 的相对目录
function M.build_dir(workspace, root)
  if M.is_file(workspace, M.join(root, "PKGBUILD")) then return root end
  local listing = workspace:read_dir(root, 1024)
  if listing.truncated then error("AUR snapshot directory exceeds entry limit") end
  table.sort(listing.entries, function(a, b) return a.path < b.path end)
  for _, entry in ipairs(listing.entries) do
    if entry.is_dir and M.is_file(workspace, M.join(entry.path, "PKGBUILD")) then return entry.path end
  end
  error("PKGBUILD not found after fetching AUR snapshot")
end

--- 【AUR】【额外文件】选择影响构建、安装及系统服务的文件
--- @param path string 相对文件路径
--- @return boolean 是否应纳入审查
function M.should_review(path)
  local name = path:match("[^/]+$") or ""
  return extensions[name:match("^.+%.([^%.]+)$")] == true or name:sub(-8) == ".install" or name:find("sysusers", 1, true) ~= nil or name:find("tmpfiles", 1, true) ~= nil
end

--- 【AUR】【审查采集】深度最多两层、最多八十个文件；不完整证据不能授予安装许可
--- @param workspace userdata 私有目录
--- @param root string 构建文件根目录
--- @return table, boolean 审查文件数组与证据是否完整
function M.collect(workspace, root)
  local files, paths, seen, complete = sai.json.array(), {}, {}, true
  --- 【AUR】【单文件采集】记录截断与读取失败，不将不完整正文当作安全证据
  --- @param relative string 相对于构建根目录的文件
  --- @return nil 无返回值
  local function collect(relative)
    if seen[relative] then return end
    local path = M.join(root, relative)
    if not M.is_file(workspace, path) then return end
    seen[relative] = true
    local ok, content = pcall(function() return workspace:read_text(path, { max_bytes = MAX_CHARS * 4 + 4 }) end)
    if not ok then
      complete = false
      files[#files + 1] = { path = relative, content = "<non-utf8 file omitted>", truncated = false }
      return
    end
    local truncated = content.truncated or utf8.len(content.text) > MAX_CHARS
    local ending = utf8.offset(content.text, MAX_CHARS + 1)
    files[#files + 1] = { path = relative, content = ending and content.text:sub(1, ending - 1) or content.text, truncated = truncated }
    complete = complete and not truncated
  end
  --- 【AUR】【有限遍历】只读取指定深度的构建目录，保留条数截断状态
  --- @param directory string 相对目录
  --- @param depth number 剩余层数
  --- @return nil 无返回值
  local function walk(directory, depth)
    if depth == 0 then return end
    local listing = workspace:read_dir(directory, 1024)
    complete = complete and not listing.truncated
    for _, entry in ipairs(listing.entries) do
      if entry.is_dir then walk(entry.path, depth - 1)
      elseif entry.is_file then paths[#paths + 1] = root == "." and entry.path or entry.path:sub(#root + 2)
      else complete = false end
    end
  end
  collect("PKGBUILD")
  collect(".SRCINFO")
  walk(root, 2)
  table.sort(paths)
  for _, path in ipairs(paths) do
    if M.should_review(path) and not seen[path] then
      if #files >= MAX_FILES then complete = false; break end
      collect(path)
    end
  end
  return files, complete
end

--- 【AUR】【构建产物】只选择普通包文件，排除签名文件和选项形式的名称
--- @param workspace userdata 私有目录
--- @param root string 构建目录
--- @return string 可作为单个 pacman 参数的相对路径
function M.built_package(workspace, root)
  local listing = workspace:read_dir(root, 1024)
  if listing.truncated then error("AUR build output exceeds entry limit") end
  table.sort(listing.entries, function(a, b) return a.name < b.name end)
  for _, entry in ipairs(listing.entries) do
    if entry.is_file and entry.name:find(".pkg.tar", 1, true) and not entry.name:match("%.sig$") then
      return "./" .. entry.name
    end
  end
  error("makepkg did not produce a package archive")
end

return M
