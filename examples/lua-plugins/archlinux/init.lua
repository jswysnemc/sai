local packages = require("packages")
local status = require("status")
local wiki = require("wiki")

sai.register_tool({
    name = "aur_search_packages",
    description = "Search AUR packages via official RPC.",
    parameters = {
        type = "object",
        properties = {
            query = { type = "string" },
            limit = { type = "integer" },
            search_by = { type = "string" },
        },
        required = { "query" }, additionalProperties = false,
    },
    access = "read_only",
    execute = packages.search,
})

sai.register_tool({
    name = "aur_get_package_info",
    description = "Get AUR package information via official RPC.",
    parameters = {
        type = "object",
        properties = { package_name = { type = "string" } },
        required = { "package_name" }, additionalProperties = false,
    },
    access = "read_only",
    execute = packages.info,
})

sai.register_tool({
    name = "archlinux_official_package_query",
    description = "Query official Arch Linux package database. Supports search and exact package details. / 查询 Arch Linux 官方软件包数据库，支持搜索和精确包详情。",
    parameters = {
        type = "object",
        properties = {
            package_name = { type = "string", description = "Package name. / 包名。" },
            repo = { type = "string", description = "Repository for detail mode, e.g. core or extra. / 详情模式的仓库，例如 core 或 extra。" },
            arch = { type = "string", description = "Architecture for detail mode, default x86_64. / 详情模式架构，默认 x86_64。" },
            mode = { type = "string", enum = { "auto", "search", "detail" }, description = "auto uses detail when repo is provided, otherwise search. / auto 在提供 repo 时查详情，否则搜索。" },
        },
        required = { "package_name" }, additionalProperties = false,
    },
    access = "read_only",
    execute = packages.official,
})

sai.register_tool({
    name = "aur_check_status",
    description = "Check Arch Linux / AUR service status with detailed incident, degradation, and downtime info.",
    parameters = { type = "object", properties = {}, additionalProperties = false },
    access = "read_only",
    execute = status.query,
})

sai.register_tool({
    name = "archwiki_query",
    description = "Search or read ArchWiki pages.",
    parameters = {
        type = "object",
        properties = {
            query = { type = "string" },
            title = { type = "string" },
            mode = { type = "string", enum = { "auto", "search", "page" } },
        },
        additionalProperties = false,
    },
    access = "read_only",
    execute = wiki.query,
})
