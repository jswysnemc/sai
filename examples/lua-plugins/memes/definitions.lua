return sai.json.decode([=[
[
  {
    "name": "search_meme",
    "description": "Search the current persona's meme library by scene, mood, tags, or visible content. Use before showing a meme unless the user provided a specific meme id.",
    "parameters": {
      "type": "object",
      "properties": {
        "query": {
          "type": "string",
          "description": "Scene, mood, visible content, or user intent."
        },
        "tags": {
          "type": "array",
          "items": {
            "type": "string"
          },
          "description": "Optional preferred tags."
        },
        "library": {
          "type": "string",
          "description": "Optional meme library override."
        },
        "limit": {
          "type": "integer",
          "description": "Maximum number of candidates, default 6."
        }
      },
      "additionalProperties": false
    },
    "writes": false
  },
  {
    "name": "show_meme",
    "description": "Render a meme in the terminal with terminal image protocols or an ANSI fallback. GIFs are shown as static previews unless animation is explicitly allowed in config.",
    "parameters": {
      "type": "object",
      "properties": {
        "id": {
          "type": "string",
          "description": "Meme sha256 id."
        },
        "library": {
          "type": "string",
          "description": "Optional meme library override."
        },
        "size": {
          "type": "string",
          "description": "Optional terminal size, e.g. 40x15."
        },
        "width": {
          "type": "integer",
          "description": "Optional output width in terminal cells."
        },
        "height": {
          "type": "integer",
          "description": "Optional output height in terminal cells."
        }
      },
      "required": [
        "id"
      ],
      "additionalProperties": false
    },
    "writes": false
  },
  {
    "name": "recent_meme",
    "description": "Get the most recent meme automatically sent for the current persona/library.",
    "parameters": {
      "type": "object",
      "properties": {},
      "additionalProperties": false
    },
    "writes": false
  },
  {
    "name": "add_meme",
    "description": "Add a local image to the current persona's writable meme library. If metadata is not supplied, the tool asks the configured vision model to generate it from the image.",
    "parameters": {
      "type": "object",
      "properties": {
        "image": {
          "type": "string",
          "description": "Local image path."
        },
        "library": {
          "type": "string",
          "description": "Optional meme library override."
        },
        "name_zh": {
          "type": "string",
          "description": "Chinese display name."
        },
        "name_en": {
          "type": "string",
          "description": "English display name."
        },
        "description": {
          "type": "string",
          "description": "Visible content description."
        },
        "usage": {
          "type": "string",
          "description": "When to use this meme."
        },
        "avoid": {
          "type": "string",
          "description": "When not to use this meme."
        },
        "tags": {
          "type": "array",
          "items": {
            "type": "string"
          },
          "description": "Search tags."
        }
      },
      "required": [
        "image"
      ],
      "additionalProperties": false
    },
    "writes": true
  },
  {
    "name": "update_meme",
    "description": "Update meme index metadata in the writable overlay for the current library.",
    "parameters": {
      "type": "object",
      "properties": {
        "id": {
          "type": "string",
          "description": "Meme sha256 id."
        },
        "library": {
          "type": "string",
          "description": "Optional meme library override."
        },
        "name_zh": {
          "type": "string"
        },
        "name_en": {
          "type": "string"
        },
        "description": {
          "type": "string"
        },
        "usage": {
          "type": "string"
        },
        "avoid": {
          "type": "string"
        },
        "tags": {
          "type": "array",
          "items": {
            "type": "string"
          }
        },
        "enabled": {
          "type": "boolean",
          "description": "Enable or disable this meme."
        }
      },
      "required": [
        "id"
      ],
      "additionalProperties": false
    },
    "writes": true
  },
  {
    "name": "delete_meme",
    "description": "Delete a user meme or disable a built-in meme in the current library.",
    "parameters": {
      "type": "object",
      "properties": {
        "id": {
          "type": "string",
          "description": "Meme sha256 id."
        },
        "library": {
          "type": "string",
          "description": "Optional meme library override."
        },
        "hard_delete": {
          "type": "boolean",
          "description": "Permanently remove user image instead of moving it to trash."
        }
      },
      "required": [
        "id"
      ],
      "additionalProperties": false
    },
    "writes": true
  }
]
]=])
