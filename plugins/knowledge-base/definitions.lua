return sai.json.decode([=[
[
  {
    "name": "search_knowledge_base",
    "description": "Search the local knowledge base content. Returns file paths and original text snippets. Use read_knowledge_base_file if snippets are insufficient. Mention paths only when useful or when the user asks.",
    "parameters": {
      "type": "object",
      "properties": {
        "query": {
          "type": "string",
          "description": "Search keywords or user question."
        },
        "max_results": {
          "type": "integer",
          "description": "Optional result limit."
        }
      },
      "required": [
        "query"
      ],
      "additionalProperties": false
    },
    "writes": false
  },
  {
    "name": "search_knowledge_base_by_name",
    "description": "Find knowledge base files by file name, directory, extension, or path fragment. Returns relative paths for read_knowledge_base_file. Mention paths only when useful or when the user asks.",
    "parameters": {
      "type": "object",
      "properties": {
        "file_name_query": {
          "type": "string",
          "description": "File name, directory, extension, or path fragment."
        },
        "max_results": {
          "type": "integer",
          "description": "Optional result limit."
        }
      },
      "required": [
        "file_name_query"
      ],
      "additionalProperties": false
    },
    "writes": false
  },
  {
    "name": "read_knowledge_base_file",
    "description": "Read a knowledge base file by relative path with line pagination. Prefer paths returned by search_knowledge_base or search_knowledge_base_by_name. Summarize the relevant content without exposing raw tool JSON.",
    "parameters": {
      "type": "object",
      "properties": {
        "file_name": {
          "type": "string",
          "description": "Knowledge base relative path."
        },
        "start_line": {
          "type": "integer",
          "description": "1-based start line."
        },
        "max_lines": {
          "type": "integer",
          "description": "Optional line limit."
        }
      },
      "required": [
        "file_name"
      ],
      "additionalProperties": false
    },
    "writes": false
  },
  {
    "name": "upload_text_to_knowledge_base",
    "description": "Create a new knowledge-base file or replace an entire existing file. For updating part of an existing file, first search/read it and prefer edit_knowledge_base_file. Never use this for skills, memory, persona, identity, or configuration.",
    "parameters": {
      "type": "object",
      "properties": {
        "content": {
          "type": "string",
          "description": "Text content to save."
        },
        "title": {
          "type": "string",
          "description": "Optional title used for markdown heading and default file name."
        },
        "file_name": {
          "type": "string",
          "description": "Optional knowledge base relative path."
        }
      },
      "required": [
        "content"
      ],
      "additionalProperties": false
    },
    "writes": true
  },
  {
    "name": "edit_knowledge_base_file",
    "description": "Edit an existing knowledge-base file by replacing an inclusive 1-based line range. Use after search_knowledge_base/read_knowledge_base_file identifies the exact file and line numbers. This updates metadata and refreshes semantic indexing when embeddings are enabled.",
    "parameters": {
      "type": "object",
      "properties": {
        "file_name": {
          "type": "string",
          "description": "Knowledge base relative path to edit."
        },
        "start_line": {
          "type": "integer",
          "description": "1-based first line to replace."
        },
        "end_line": {
          "type": "integer",
          "description": "1-based last line to replace, inclusive."
        },
        "replacement": {
          "type": "string",
          "description": "Replacement text. May contain multiple lines. Empty text deletes the line range."
        }
      },
      "required": [
        "file_name",
        "start_line",
        "end_line",
        "replacement"
      ],
      "additionalProperties": false
    },
    "writes": true
  },
  {
    "name": "remove_knowledge_base_file",
    "description": "Remove a knowledge-base file by relative path. Use only after the user asks to delete a knowledge-base entry or confirms the exact file. This also removes its metadata and semantic chunks.",
    "parameters": {
      "type": "object",
      "properties": {
        "file_name": {
          "type": "string",
          "description": "Knowledge base relative path to remove."
        }
      },
      "required": [
        "file_name"
      ],
      "additionalProperties": false
    },
    "writes": true
  }
]
]=])
