local query = require("query")

sai.register_tool({
    name="query_moegirl",
    description="Search or read Moegirlpedia pages. Supports zh/cn, uk, and ja sites.",
    access="read_only",
    parameters={type="object", properties={
        query={type="string"}, title={type="string"},
        mode={type="string", enum=sai.json.array({"auto", "search", "page"})},
        site={type="string", enum=sai.json.array({"zh", "cn", "uk", "ja", ""})},
    }, additionalProperties=false},
    execute=query.run,
})
