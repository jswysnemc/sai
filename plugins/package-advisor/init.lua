local review = require("review")
local install = require("install")

sai.register_tool({
  name = "review_aur_package",
  description = "Fetch AUR build files and prepare a PKGBUILD security review. After review, stop and ask the user whether to install; do not call install_aur_package in the same turn.",
  parameters = {
    type = "object", properties = { package = { type = "string", description = "AUR package name." } },
    required = sai.json.array({ "package" }), additionalProperties = false,
  },
  execute = review.run,
})

sai.register_tool({
  name = "install_aur_package", access = "writes",
  description = "Install an AUR package only after review_aur_package recorded an allowed review state and the user explicitly confirmed installation in a later reply. Requires user_confirmed=true. Tries paru, then yay, then AUR snapshot + makepkg + pacman -U fallback.",
  parameters = {
    type = "object", properties = {
      package = { type = "string", description = "AUR package name." },
      user_confirmed = { type = "boolean", description = "Set true only when the user explicitly confirmed installation after seeing the review." },
    },
    required = sai.json.array({ "package", "user_confirmed" }), additionalProperties = false,
  },
  execute = install.run,
})
