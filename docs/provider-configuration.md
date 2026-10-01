# Provider configuration

[简体中文](provider-configuration.zh-CN.md) | [README](../README.md)

## First use

A new installation opens provider setup before the first interactive `sai` session or after entering the `sai web` workbench. Choose a preset or a custom provider, enter the API address and credentials, and specify a default model. The built-in free opencode Zen preset does not require your own key.

Saving completes setup for both interfaces and selects the provider/model for new conversations. Setup validates fields and credential references without sending a model request. Cancelling the terminal wizard exits that launch and keeps setup pending. Configurations created by earlier versions open normally; upgrading does not require repeating setup.

For providers that require authentication, use a key or `$env:VARIABLE_NAME`. The variable must exist in the environment of the Sai process, including when Sai runs as a service or in a container.

## Web settings

Open **Settings → LLM providers → Connection** or `/settings/providers/connection`.

1. Add a provider or select an existing one. Enter the API address, protocol and credentials.
2. Set its display name. A new provider's ID follows the name until you enter a separate ID. ID changes apply when the field loses focus or you press Enter; empty or duplicate IDs are rejected.
3. Use **Import models**, or add model IDs on the **Models** tab. Select the default model on **Connection**.
4. Click **Save**. Use **Set as current** when this provider should become the current default, then save that change too.

Changing a name or ID keeps the same provider selected. Saved keys and configuration references remain associated with that provider, including when another edit occurs during a save. API keys stay masked unless you explicitly view or edit them. If the provider's credentials are stored separately in `secrets.jsonc`, its `api_keys` entry is keyed by provider ID; update that entry when changing the ID.

**Test connection** sends a minimal chat request. **Test tools** checks whether the model emits a tool call. These checks do not depend on a `/models` endpoint. They use the current draft, so you can test before saving.

## Terminal settings

Run `sai config`, or enter `/config` in a conversation. Open **Providers and models**. Press `a` in the provider column to add a provider, or Enter to edit the selected provider.

The provider editor separates connection, credentials and advanced request settings. **Test and import models** explicitly fetches the current draft's catalog. **Save provider** also imports models when you add an enabled provider, change its connection or credentials, or edit a provider without configured models. Existing local models are retained, and the first imported model becomes the default only when no default is configured.

**Save provider** applies the draft inside the configurator. Return to the main menu, choose **Save and exit**, and confirm the save to write it to disk. Back in the same conversation, `/model` reads the saved configuration and lists the imported models; no application restart is needed.

Browsing a remote catalog alone does not activate all of its models. In the model column, Tab toggles a model's activation, and `a` adds a model ID manually. Save and exit after these changes.

## When a model is missing

| Symptom | Check |
| --- | --- |
| `/model` cannot find a newly configured model | Finish **Save provider**, then **Save and exit** in the main menu. Confirm the provider is enabled. |
| The provider does not implement `/models` | Add its model ID manually. A failed import retains existing local models. |
| A new provider has no models after an import failure | It is saved without replacing the previously active provider. Add a model in the model column, then save. |
| An environment variable is not set | Export it in the Sai process environment and restart that process, or explicitly enter the key in the credentials editor. |
| The Web page still exhibits the old behavior after upgrading | Restart `sai web` with the new binary and reload the page. Back up configuration before replacing it manually. |

To use another configured model, run `/model`, type to filter, and press Enter. Disabled providers do not appear in the list.
