local settings = sai.config or {}
assert(type(settings) == "table", "exchange-rate settings must be an object")
assert(settings.api_key == nil or type(settings.api_key) == "string", "api_key must be a string")
assert(settings.free_fallback_enabled == nil or type(settings.free_fallback_enabled) == "boolean",
    "free_fallback_enabled must be a boolean")

local fallback = settings.free_fallback_enabled
if fallback == nil then fallback = true end

return {api_key=sai.text.trim(settings.api_key or ""), free_fallback_enabled=fallback}
