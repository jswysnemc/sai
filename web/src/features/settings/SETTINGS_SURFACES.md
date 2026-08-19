# Settings surfaces contract

This document freezes the Web settings information architecture used by the registry.

## AppConfig participation

Every section declares one `appConfig` value in `settings-registry.ts`. Topbar save,
loading skeleton, and the error banner are all derived from this single field —
no per-section special cases.

| appConfig | Meaning | Topbar | Loading gate |
|---|---|---|---|
| `required` | Reads and writes global `AppConfig` | Save button always shown | Skeleton until config loads; error recovery on failure |
| `optional` | Standalone features that may also write `AppConfig` fields | Save button only while dirty, hint otherwise | None; section degrades when config is absent |
| `none` | Never touches `AppConfig` | Save hint only (`saveHintEn/Zh`) | None |

Derivation rules (implemented in `showsAppConfigSave` and `SettingsSectionBody`):

- Topbar save: `required`, or `optional && dirty`
- Skeleton / error recovery: `required` only
- Global error banner: `required` and `optional`

## Section inventory

| id | group | appConfig | Data source | Notes |
|---|---|---|---|---|
| providers | general | required | `api.config` | Endpoints, credentials, models. Connection test sends a minimal chat; tool test sends a dummy function definition. Catalog refresh is a separate action, not a probe prerequisite |
| agents | general | required | `api.config` + agent APIs | Profile workspace may call agent APIs |
| runtime | general | required | `api.config` | Engine, permissions, notifications, terminal, context, display, tools |
| appearance | general | none | theme/locale local storage | Instant apply, browser-only |
| prompts | general | required | `api.config` | Internal prompt templates |
| cli-tools | integrations | required | `api.config` | Optional CLI assistant tools (route alias: `plugins`) |
| web-search | integrations | required | `api.config` | Search providers and credentials |
| skills | integrations | optional | skills filesystem APIs + `AppConfig.skills` | Documents save instantly; behavior fields via topbar save |
| mcp | integrations | none | `api.config` MCP endpoints | Separate document, section-local save |
| hooks | integrations | required | `api.config` | Lifecycle hooks |
| gateways | integrations | required | `api.config` | Section additionally passes dirty/onSave to its runtime controls |
| git | workspace | required | `api.config` | SCM safety and defaults |
| memory | operations | optional | memory APIs + `AppConfig.plugins.memory` | Facts/events act instantly; config fields via topbar save |
| session-data | operations | none | session data APIs | Inspect and delete workspace sessions |
| usage | operations | none | usage APIs | Read-only stats |
| advanced | advanced | required | raw AppConfig JSON | Escape hatch |

## Groups

| group | en | zh |
|---|---|---|
| general | General | 常用配置 |
| integrations | Extensions | 扩展与集成 |
| workspace | Workspace | 工作区 |
| operations | Data and ops | 数据与运维 |
| advanced | Advanced | 高级 |

## Routing

- `/settings` redirects to `/settings/providers`
- `/settings/:sectionId` opens a registered section
- Unknown sectionId redirects to `/settings/providers`
- Legacy alias: `plugins` redirects to `cli-tools`

## Skills boundary

- Skill **documents** (scan/create/edit/enable) live under Skills operations UI.
- Skill **behavior** fields (`AppConfig.skills`, e.g. progressive loading / command execution) are edited on the Skills page and saved via top-bar AppConfig Save when dirty.
- Runtime links to Skills instead of duplicating the skills structured group.
