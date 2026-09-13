local definitions = sai.json.decode([=[
{
  "en": {
    "name": "todo",
    "description": "Track a multi-step plan for the current session. Actions: list reads all items with 1-based indexes; add creates one item (text) or several at once (texts array), optionally inserting before position index; update changes an item's text or status; remove deletes an item. update and remove locate the item by id or by 1-based index. Status flows pending -> in_progress -> completed (or cancelled). Items advance in order: finish earlier items before advancing later ones and keep at most one in_progress; completing a pending item directly is allowed once all earlier items are finished. When every item is completed or cancelled, the plan is archived to history and the active list is cleared. Every mutating call returns the full active items snapshot, so you rarely need a separate list call. Prefer creating the whole plan in a single add call with texts. Use this for tasks with three or more steps; skip it for trivial single-step work.",
    "parameters": {
      "type": "object",
      "properties": {
        "action": {
          "type": "string",
          "enum": [
            "list",
            "add",
            "update",
            "remove"
          ],
          "description": "Which operation to perform."
        },
        "id": {
          "type": "string",
          "description": "Item id for update or remove. Obtain it from any previous result; index is an alternative."
        },
        "index": {
          "type": "integer",
          "description": "1-based position. For add: insert before this position (defaults to appending). For update or remove: the target item when id is absent."
        },
        "text": {
          "type": "string",
          "description": "Single item text. Required for add unless texts is given; optional for update to rename."
        },
        "texts": {
          "type": "array",
          "items": {
            "type": "string"
          },
          "description": "Multiple item texts for add, created in order in one call."
        },
        "status": {
          "type": "string",
          "enum": [
            "pending",
            "in_progress",
            "completed",
            "cancelled"
          ],
          "description": "New status for update. Advance items in order and keep at most one in_progress."
        }
      },
      "required": [
        "action"
      ],
      "additionalProperties": false
    },
    "writes": true
  },
  "zh": {
    "name": "todo",
    "description": "跟踪当前会话的多步计划。动作：list 读取全部条目并附 1 起始序号；add 创建单条(text)或一次创建多条(texts 数组)，可用 index 指定插入位置；update 修改条目文本或状态；remove 删除条目。update 与 remove 通过 id 或 1 起始的 index 定位条目。状态流转为 pending -> in_progress -> completed（或 cancelled）。条目按顺序推进：先完成前面的条目再推进后面的，同一时刻至多一个 in_progress；当前面条目全部完成时，允许把 pending 条目直接标记为 completed。计划全部完成后立即归档到历史并清空活动列表。每次修改都会返回完整活动清单快照，一般无需再单独调用 list。建议用一次 add 携带 texts 创建完整计划。任务达到三步及以上时使用，单步琐碎任务不必使用。",
    "parameters": {
      "type": "object",
      "properties": {
        "action": {
          "type": "string",
          "enum": [
            "list",
            "add",
            "update",
            "remove"
          ],
          "description": "要执行的操作。"
        },
        "id": {
          "type": "string",
          "description": "update 或 remove 的条目 id，可从任意先前结果获取；也可以改用 index 定位。"
        },
        "index": {
          "type": "integer",
          "description": "1 起始的序号。add 时表示插入到该位置之前（缺省追加到末尾）；update 或 remove 未提供 id 时用它定位目标条目。"
        },
        "text": {
          "type": "string",
          "description": "单条内容。add 时未提供 texts 则必填；update 时可选，用于改写文本。"
        },
        "texts": {
          "type": "array",
          "items": {
            "type": "string"
          },
          "description": "add 的批量内容，一次调用按顺序创建多条。"
        },
        "status": {
          "type": "string",
          "enum": [
            "pending",
            "in_progress",
            "completed",
            "cancelled"
          ],
          "description": "update 的新状态。按顺序推进，同时至多一个 in_progress。"
        }
      },
      "required": [
        "action"
      ],
      "additionalProperties": false
    },
    "writes": true
  }
}
]=])

local selected = definitions[sai.config.language == "zh" and "zh" or "en"]
return {name=selected.name, description=selected.description, parameters=selected.parameters, access="writes"}
