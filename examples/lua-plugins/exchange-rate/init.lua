local query = require("query")

sai.register_tool({
    name="get_exchange_rate",
    description="Query exchange rate between two currencies. Supports ISO codes such as USD/EUR/JPY and common Chinese names.",
    access="read_only",
    parameters={type="object", properties={
        base={type="string", description="Base currency, e.g. USD or 美元."},
        target={type="string", description="Target currency, e.g. JPY or 日元."},
    }, required=sai.json.array({"base", "target"}), additionalProperties=false},
    execute=query.run,
})
