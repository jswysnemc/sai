local settings = sai.config
assert(type(settings) == "table", "reply-notification settings must be an object")
for name, value in pairs(settings) do
    assert(name == "enabled" or name == "sound", "unknown reply-notification setting: " .. tostring(name))
    assert(type(value) == "boolean", name .. " must be a boolean")
end

local enabled = settings.enabled
local sound = settings.sound
if enabled == nil then enabled = true end
if sound == nil then sound = true end

return {enabled=enabled, sound=sound}
