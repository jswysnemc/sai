local collect = require("collect")
local app_probe = require("app_probe")
local properties = {
    query={type="string", description="Original issue used for automatic area and target inference."},
    area={type="string", enum=sai.json.array({"auto", "system", "app", "input_method", "display", "audio", "package", "package_update", "gpu", "network", "storage"})},
    mode={type="string", description="Legacy alias for area; area takes precedence."},
    target={type="string", description="Optional application, process, command, package, or subsystem target."},
    symptom={type="string", description="Optional symptom label."},
    depth={type="string", enum=sai.json.array({"quick", "normal", "full"})},
    recent_minutes={type="integer", description="Recent log window, clamped to 1..1440 minutes."},
    platform={type="string", enum=sai.json.array({"auto", "linux", "macos"}), description="Platform override. Prefer auto."},
    allow_launch_probe={type="boolean", description="Legacy option: true is rejected by check_issue. Use diagnostic_app_probe with probe=launch."},
    launch_timeout_seconds={type="integer", description="Explicit launch sampling delay, clamped to 1..15 seconds; default 3."},
}

sai.register_tool({
    name="check_issue",
    description="Collect read-only local diagnostic evidence without executing target applications. Keeps Linux evidence across nine areas and basic macOS evidence; use diagnostic_app_probe for explicit launch or --version execution.",
    access="read_only",
    parameters={type="object", properties=properties, required=sai.json.array(), additionalProperties=false},
    execute=collect.run,
})

local probe_properties = {}
for name, definition in pairs(properties) do probe_properties[name] = definition end
probe_properties.probe = {type="string", enum=sai.json.array({"version", "launch"}), description="Explicit action: execute target --version, or launch and sample the target."}
sai.register_tool({
    name="diagnostic_app_probe",
    description="Explicitly execute a target application's --version or launch it for diagnostic sampling. Requires write permission. Long-running launches return a managed background task ID that can be stopped.",
    access="writes",
    parameters={type="object", properties=probe_properties, required=sai.json.array({"probe"}), additionalProperties=false},
    execute=app_probe.run,
})
