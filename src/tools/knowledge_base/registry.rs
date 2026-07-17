use super::{ToolRegistry, ToolSpec};
use crate::config::{AppConfig, KnowledgeBasePluginConfig, ProviderConfig};
use crate::paths::SaiPaths;
use anyhow::{bail, Context, Result};
use chrono::Local;
use reqwest::Client;
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::process::Command;

pub fn register(registry: &mut ToolRegistry, config: AppConfig, paths: SaiPaths) {
    register_readonly(registry, config.clone(), paths.clone());
    if config.plugins.knowledge_base.upload_tool_enabled {
        let upload_config = config.clone();
        let upload_paths = paths.clone();
        registry.register(ToolSpec::new(
                "upload_text_to_knowledge_base",
            "Create a new knowledge-base file or replace an entire existing file. For updating part of an existing file, first search/read it and prefer edit_knowledge_base_file. Never use this for skills, memory, persona, identity, or configuration.",
            json!({
                "type": "object",
                "properties": {
                    "content": { "type": "string", "description": "Text content to save." },
                    "title": { "type": "string", "description": "Optional title used for markdown heading and default file name." },
                    "file_name": { "type": "string", "description": "Optional knowledge base relative path." }
                },
                "required": ["content"],
                "additionalProperties": false
            }),
            move |args| {
                let config = upload_config.clone();
                let paths = upload_paths.clone();
                async move { tool_upload(args, config, paths).await }
            },
        ).writes());
        let edit_config = config.clone();
        let edit_paths = paths.clone();
        registry.register(ToolSpec::new(
            "edit_knowledge_base_file",
            "Edit an existing knowledge-base file by replacing an inclusive 1-based line range. Use after search_knowledge_base/read_knowledge_base_file identifies the exact file and line numbers. This updates metadata and refreshes semantic indexing when embeddings are enabled.",
            json!({
                "type": "object",
                "properties": {
                    "file_name": { "type": "string", "description": "Knowledge base relative path to edit." },
                    "start_line": { "type": "integer", "description": "1-based first line to replace." },
                    "end_line": { "type": "integer", "description": "1-based last line to replace, inclusive." },
                    "replacement": { "type": "string", "description": "Replacement text. May contain multiple lines. Empty text deletes the line range." }
                },
                "required": ["file_name", "start_line", "end_line", "replacement"],
                "additionalProperties": false
            }),
            move |args| {
                let config = edit_config.clone();
                let paths = edit_paths.clone();
                async move { tool_edit(args, config, paths).await }
            },
        ).writes());
        let remove_config = config.clone();
        let remove_paths = paths.clone();
        registry.register(ToolSpec::new(
            "remove_knowledge_base_file",
            "Remove a knowledge-base file by relative path. Use only after the user asks to delete a knowledge-base entry or confirms the exact file. This also removes its metadata and semantic chunks.",
            json!({
                "type": "object",
                "properties": {
                    "file_name": { "type": "string", "description": "Knowledge base relative path to remove." }
                },
                "required": ["file_name"],
                "additionalProperties": false
            }),
            move |args| {
                let config = remove_config.clone();
                let paths = remove_paths.clone();
                async move { tool_remove(args, config, paths).await }
            },
        ).writes());
    }
}

pub fn register_readonly(registry: &mut ToolRegistry, config: AppConfig, paths: SaiPaths) {
    registry.register(ToolSpec::new(
        "search_knowledge_base",
        "Search the local knowledge base content. Returns file paths and original text snippets. Use read_knowledge_base_file if snippets are insufficient. Mention paths only when useful or when the user asks.",
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Search keywords or user question." },
                "max_results": { "type": "integer", "description": "Optional result limit." }
            },
            "required": ["query"],
            "additionalProperties": false
        }),
        {
            let config = config.clone();
            let paths = paths.clone();
            move |args| {
                let config = config.clone();
                let paths = paths.clone();
                async move { tool_search_readonly(args, config, paths).await }
            }
        },
    ));
    registry.register(ToolSpec::new(
        "search_knowledge_base_by_name",
        "Find knowledge base files by file name, directory, extension, or path fragment. Returns relative paths for read_knowledge_base_file. Mention paths only when useful or when the user asks.",
        json!({
            "type": "object",
            "properties": {
                "file_name_query": { "type": "string", "description": "File name, directory, extension, or path fragment." },
                "max_results": { "type": "integer", "description": "Optional result limit." }
            },
            "required": ["file_name_query"],
            "additionalProperties": false
        }),
        {
            let config = config.clone();
            let paths = paths.clone();
            move |args| {
                let config = config.clone();
                let paths = paths.clone();
                async move { tool_find_readonly(args, config, paths).await }
            }
        },
    ));
    registry.register(ToolSpec::new(
        "read_knowledge_base_file",
        "Read a knowledge base file by relative path with line pagination. Prefer paths returned by search_knowledge_base or search_knowledge_base_by_name. Summarize the relevant content without exposing raw tool JSON.",
        json!({
            "type": "object",
            "properties": {
                "file_name": { "type": "string", "description": "Knowledge base relative path." },
                "start_line": { "type": "integer", "description": "1-based start line." },
                "max_lines": { "type": "integer", "description": "Optional line limit." }
            },
            "required": ["file_name"],
            "additionalProperties": false
        }),
        {
            let config = config.clone();
            let paths = paths.clone();
            move |args| {
                let config = config.clone();
                let paths = paths.clone();
                async move { tool_read_readonly(args, config, paths).await }
            }
        },
    ));
}

