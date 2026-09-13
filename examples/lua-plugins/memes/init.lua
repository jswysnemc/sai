local config = require("settings").load(sai.config or {})
require("tools").register(config)
require("reply").register(config)
