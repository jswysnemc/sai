local config = require("settings").resolve(sai.config)
require("tools").register(config)
require("commands").register(config)
