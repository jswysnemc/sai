local query = require("query")

sai.register_tool({
    name="get_weather",
    description="Query current weather via wttr.in. Use for weather questions. Location can be city name, airport code, or empty for auto-detected location.",
    access="read_only",
    parameters={type="object", properties={
        location={type="string", description="City/location, for example Tokyo or Beijing. Empty means auto-detect."},
    }, additionalProperties=false},
    execute=query.run,
})
