# Settings surfaces contract

This document freezes the Web settings information architecture used by the registry.

## Surface kinds

| kind | Meaning | Topbar Save | Dirty source |
|---|---|---|---|
| `app-config` | Part of global `AppConfig` | Show global Save | `useSettingsConfig.dirty` |
| `local-config` | Independent config document | Hide global Save; section owns save | Section-local dirty |
| `client-pref` | Browser-only preference | Hide | Instant apply |
| `operations` | Operational actions / lists | Hide | N/A |
| `analytics` | Read-only stats | Hide | N/A |

## Section inventory

| id | group | kind | Data source | Save model | Notes |
|---|---|---|---|---|---|
| providers | general | app-config | `api.config` | global | Endpoints, credentials, models |
| agents | general | app-config | `api.config` + agents APIs | global for profile fields | Profile workspace may call agent APIs |
| runtime | general | app-config | `api.config` | global | Permissions, notifications, terminal, context, display, tools |
| appearance | general | client-pref | theme/locale local storage | instant | Does not mutate server config |
| plugins | integrations | app-config | `api.config` | global | Search/vision/knowledge/memory plugins |
| skills | integrations | operations | skills filesystem APIs | action buttons | Resource manager, not AppConfig form |
| mcp | integrations | local-config | `api.config` MCP endpoints | section Save MCP | Separate dirty from AppConfig |
| hooks | integrations | app-config | `api.config` | global | Lifecycle hooks |
| gateways | integrations | app-config | `api.config` | global | Credentials; runtime status lives on `/gateways` |
| git | workspace | app-config | `api.config` | global | SCM safety and defaults |
| memory | operations | operations | memory APIs | action buttons | Facts/events reset |
| usage | operations | analytics | usage APIs | none | Token trends and logs |
| advanced | advanced | app-config | raw AppConfig JSON | global | Escape hatch |

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


## Skills boundary

- Skill **documents** (scan/create/edit/enable) live under Skills operations UI.
- Skill **behavior** fields (`AppConfig.skills`, e.g. progressive loading / command execution) are edited on the Skills page and saved via top-bar AppConfig Save when dirty.
- Runtime links to Skills instead of duplicating the skills structured group.
