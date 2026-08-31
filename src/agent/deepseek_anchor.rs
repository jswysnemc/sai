use crate::llm::{FunctionDefinition, ToolCall, ToolCallFunction, ToolDefinition};
use crate::tools::fs_path::{expand_path, fs_error};
use anyhow::{bail, Context, Result};
use serde_json::{json, Map, Value};
use std::path::Path;

pub(crate) const BASH_NAME: &str = "bash";
pub(crate) const EDITOR_NAME: &str = "str_replace_editor";
pub(crate) const BASH_EXECUTION_KIND: &str = "sai-dsh-bash";

const BASH_DESCRIPTION: &str = "Run commands in a bash shell\n\
* When invoking this tool, the contents of the \"command\" parameter does NOT need to be XML-escaped.\n\
* You don't have access to the internet via this tool.\n\
* You do have access to a mirror of common linux and python packages via apt and pip.\n\
* State is persistent across command calls and discussions with the user.\n\
* To inspect a particular line range of a file, e.g. lines 10-25, try 'sed -n 10,25p /path/to/the/file'.\n\
* Please avoid commands that may produce a very large amount of output.\n\
* Please run long lived commands in the background, e.g. 'sleep 10 &' or start a server in the background.";

const EDITOR_DESCRIPTION: &str = "Custom editing tool for viewing, creating and editing files\n\
* State is persistent across command calls and discussions with the user\n\
* If `path` is a file, `view` displays the result of applying `cat -n`. If `path` is a directory, `view` lists non-hidden files and directories up to 2 levels deep\n\
* The `create` command cannot be used if the specified `path` already exists as a file\n\
* If a `command` generates a long output, it will be truncated and marked with `<response clipped>`\n\n\
Notes for using the `str_replace` command:\n\
* The `old_str` parameter should match EXACTLY one or more consecutive lines from the original file. Be mindful of whitespaces!\n\
* If the `old_str` parameter is not unique in the file, the replacement will not be performed. Make sure to include enough context in `old_str` to make it unique\n\
* The `new_str` parameter should contain the edited lines that should replace the `old_str`";

/// Return the provider-facing pair from DeepSeek Harness commit 47f9438's Minimal preset.
pub(crate) fn definitions() -> Vec<ToolDefinition> {
    vec![bash_definition(), editor_definition()]
}

/// Whether this is one of the dsh names exposed to the provider.
pub(crate) fn is_provider_tool(name: &str) -> bool {
    matches!(name, BASH_NAME | EDITOR_NAME)
}

/// Whether a translated dsh call may enter sai's local execution pipeline.
pub(crate) fn is_execution_tool(name: &str) -> bool {
    matches!(
        name,
        "run_command" | "read_file" | "write_file" | "str_replace"
    )
}

/// Translate a provider-visible dsh call into the equivalent registered sai tool call.
pub(crate) fn resolve_execution_call(provider_call: &ToolCall) -> Result<ToolCall> {
    match provider_call.function.name.as_str() {
        BASH_NAME => translate_bash(provider_call),
        EDITOR_NAME => translate_editor(provider_call),
        name => bail!("not a DeepSeek anchor tool: {name}"),
    }
}

fn bash_definition() -> ToolDefinition {
    definition(
        BASH_NAME,
        BASH_DESCRIPTION,
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The bash command to run. Relative path is preferred in the command."
                }
            },
            "required": ["command"]
        }),
    )
}

fn editor_definition() -> ToolDefinition {
    definition(
        EDITOR_NAME,
        EDITOR_DESCRIPTION,
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The commands to run. Allowed options are: `view`, `create`, `str_replace`, `insert`.",
                    "enum": ["view", "create", "str_replace", "insert"]
                },
                "path": {
                    "type": "string",
                    "description": "Absolute path to file or directory, e.g. `/repo/file.py` or `/repo`."
                },
                "file_text": {
                    "type": "string",
                    "description": "Required parameter of `create` command, with the content of the file to be created."
                },
                "insert_line": {
                    "type": "integer",
                    "description": "Required parameter of `insert` command. The `new_str` will be inserted AFTER the line `insert_line` of `path`."
                },
                "new_str": {
                    "type": "string",
                    "description": "Optional parameter of `str_replace` command containing the new string (if not given, no string will be added). Required parameter of `insert` command containing the string to insert."
                },
                "old_str": {
                    "type": "string",
                    "description": "Required parameter of `str_replace` command containing the string in `path` to replace."
                },
                "view_range": {
                    "type": "array",
                    "description": "Optional parameter of `view` command when `path` points to a file. If none is given, the full file is shown. If provided, the file will be shown in the indicated line number range, e.g. [11, 12] will show lines 11 and 12. Indexing at 1 to start. Setting `[start_line, -1]` shows all lines from `start_line` to the end of the file.",
                    "items": {"type": "integer"}
                }
            },
            "required": ["command", "path"]
        }),
    )
}

fn definition(name: &str, description: &str, parameters: Value) -> ToolDefinition {
    ToolDefinition {
        kind: "function",
        function: FunctionDefinition {
            name: name.to_string(),
            description: description.to_string(),
            parameters,
        },
    }
}

fn translate_bash(provider_call: &ToolCall) -> Result<ToolCall> {
    let args = validated_arguments(provider_call, &bash_definition())?;
    let command = required_string(&args, "command", false)?;
    let mut call = translated_call(provider_call, "run_command", json!({"command": command}))?;
    call.kind = BASH_EXECUTION_KIND.to_string();
    Ok(call)
}

fn translate_editor(provider_call: &ToolCall) -> Result<ToolCall> {
    let args = validated_arguments(provider_call, &editor_definition())?;
    let command = required_string(&args, "command", false)?;
    let path_text = required_string(&args, "path", false)?;
    ensure_absolute_path(path_text)?;

    match command {
        "view" => translate_view(provider_call, &args, path_text),
        "create" => translate_create(provider_call, &args, path_text),
        "str_replace" => translate_replace(provider_call, &args, path_text),
        "insert" => translate_insert(provider_call, &args, path_text),
        _ => bail!("unsupported str_replace_editor command: {command}"),
    }
}

fn translate_view(
    provider_call: &ToolCall,
    args: &Map<String, Value>,
    path: &str,
) -> Result<ToolCall> {
    let mut local = json!({"path": path});
    if let Some(range) = args.get("view_range") {
        let range = range
            .as_array()
            .context("Invalid `view_range`. It should be a list of two integers.")?;
        if range.len() != 2 {
            bail!("Invalid `view_range`. It should be a list of two integers.");
        }
        let start = range[0]
            .as_i64()
            .context("Invalid `view_range`. It should be a list of two integers.")?;
        let end = range[1]
            .as_i64()
            .context("Invalid `view_range`. It should be a list of two integers.")?;
        if start < 1 || (end != -1 && end < start) {
            bail!("Invalid `view_range`: [{start}, {end}]");
        }
        local["offset"] = json!(start);
        if end != -1 {
            local["limit"] = json!(end - start + 1);
        }
    }
    translated_call(provider_call, "read_file", local)
}

fn translate_create(
    provider_call: &ToolCall,
    args: &Map<String, Value>,
    path_text: &str,
) -> Result<ToolCall> {
    let content = required_string(args, "file_text", true)?;
    let path = expand_path(path_text);
    if path.exists() {
        bail!(
            "File already exists at: {}. Cannot overwrite files using command `create`.",
            path.display()
        );
    }
    translated_call(
        provider_call,
        "write_file",
        json!({"path": path_text, "content": content}),
    )
}

fn translate_replace(
    provider_call: &ToolCall,
    args: &Map<String, Value>,
    path: &str,
) -> Result<ToolCall> {
    let old_string = required_string(args, "old_str", false)?;
    let new_string = optional_string(args, "new_str")?.unwrap_or_default();
    translated_call(
        provider_call,
        "str_replace",
        json!({
            "path": path,
            "old_string": old_string,
            "new_string": new_string
        }),
    )
}

fn translate_insert(
    provider_call: &ToolCall,
    args: &Map<String, Value>,
    path_text: &str,
) -> Result<ToolCall> {
    let insert_line = args
        .get("insert_line")
        .and_then(Value::as_i64)
        .context("Parameter `insert_line` is required for command: insert")?;
    let new_string = required_string(args, "new_str", true)?;
    let path = expand_path(path_text);
    if !path.is_file() {
        bail!("The path {} is not a regular file", path.display());
    }
    let raw =
        std::fs::read_to_string(&path).map_err(|error| fs_error("read file", &path, &error))?;
    // sai's replacement tool presents CRLF files as LF and restores their line endings on write.
    let before = raw.replace("\r\n", "\n");
    let lines = before.split('\n').collect::<Vec<_>>();
    if insert_line < 0 || insert_line as usize > lines.len() {
        bail!(
            "Invalid `insert_line` parameter: {insert_line}. It should be within the range of lines of the file: [0, {}]",
            lines.len()
        );
    }
    let index = insert_line as usize;
    let mut after = Vec::with_capacity(lines.len() + new_string.lines().count().max(1));
    after.extend_from_slice(&lines[..index]);
    after.extend(new_string.split('\n'));
    after.extend_from_slice(&lines[index..]);
    let after = after.join("\n");

    if before.is_empty() {
        translated_call(
            provider_call,
            "write_file",
            json!({"path": path_text, "content": after}),
        )
    } else {
        translated_call(
            provider_call,
            "str_replace",
            json!({
                "path": path_text,
                "old_string": before,
                "new_string": after
            }),
        )
    }
}

fn validated_arguments(
    provider_call: &ToolCall,
    definition: &ToolDefinition,
) -> Result<Map<String, Value>> {
    let value = if provider_call.function.arguments.trim().is_empty() {
        json!({})
    } else {
        super::first_json_object(&provider_call.function.arguments)
            .context("invalid DeepSeek anchor tool arguments")?
    };
    let object = value
        .as_object()
        .context("DeepSeek anchor tool arguments must be a JSON object")?;
    let validator = jsonschema::validator_for(&definition.function.parameters)
        .context("invalid built-in DeepSeek anchor schema")?;
    validator
        .validate(&value)
        .map_err(|error| anyhow::anyhow!(error.to_string()))
        .context("arguments do not match the dsh Minimal tool schema")?;
    Ok(object.clone())
}

fn required_string<'a>(
    args: &'a Map<String, Value>,
    key: &str,
    allow_empty: bool,
) -> Result<&'a str> {
    let value = args
        .get(key)
        .and_then(Value::as_str)
        .with_context(|| format!("Parameter `{key}` is required"))?;
    if !allow_empty && value.is_empty() {
        bail!("Parameter `{key}` is empty");
    }
    Ok(value)
}

fn optional_string<'a>(args: &'a Map<String, Value>, key: &str) -> Result<Option<&'a str>> {
    args.get(key)
        .map(|value| {
            value
                .as_str()
                .with_context(|| format!("Parameter `{key}` must be a string"))
        })
        .transpose()
}

fn ensure_absolute_path(path: &str) -> Result<()> {
    if path.trim().is_empty() {
        bail!("path must be a non-empty string");
    }
    if !Path::new(path).is_absolute() {
        bail!("The path {path} is not an absolute path");
    }
    Ok(())
}

fn translated_call(provider_call: &ToolCall, name: &str, arguments: Value) -> Result<ToolCall> {
    Ok(ToolCall {
        id: provider_call.id.clone(),
        kind: provider_call.kind.clone(),
        function: ToolCallFunction {
            name: name.to_string(),
            arguments: serde_json::to_string(&arguments)?,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn definitions_match_dsh_minimal_names_and_required_fields() {
        let definitions = definitions();
        assert_eq!(
            definitions
                .iter()
                .map(|definition| definition.function.name.as_str())
                .collect::<Vec<_>>(),
            [BASH_NAME, EDITOR_NAME]
        );
        assert_eq!(
            definitions[0].function.parameters["required"],
            json!(["command"])
        );
        assert_eq!(
            definitions[1].function.parameters["required"],
            json!(["command", "path"])
        );
        assert_eq!(
            definitions[1].function.parameters["properties"]["command"]["enum"],
            json!(["view", "create", "str_replace", "insert"])
        );
        assert!(definitions[1]
            .function
            .parameters
            .get("additionalProperties")
            .is_none());

        let serialized = serde_json::to_string(&definitions[1].function.parameters).unwrap();
        let command = serialized.find("\"command\"").unwrap();
        let path = serialized.find("\"path\"").unwrap();
        let file_text = serialized.find("\"file_text\"").unwrap();
        let insert_line = serialized.find("\"insert_line\"").unwrap();
        let new_str = serialized.find("\"new_str\"").unwrap();
        let old_str = serialized.find("\"old_str\"").unwrap();
        let view_range = serialized.find("\"view_range\"").unwrap();
        assert!(command < path);
        assert!(path < file_text);
        assert!(file_text < insert_line);
        assert!(insert_line < new_str);
        assert!(new_str < old_str);
        assert!(old_str < view_range);
    }

    #[test]
    fn translates_bash_and_editor_replace_to_sai_calls() {
        let bash = resolve_execution_call(&call(BASH_NAME, r#"{"command":"pwd"}"#)).unwrap();
        assert_eq!(bash.function.name, "run_command");
        assert_eq!(bash.kind, BASH_EXECUTION_KIND);
        assert_eq!(json_args(&bash), json!({"command": "pwd"}));

        let path = std::env::temp_dir().join("anchor-replace.txt");
        let editor = resolve_execution_call(&call(
            EDITOR_NAME,
            &json!({
                "command": "str_replace",
                "path": path,
                "old_str": "old",
                "new_str": "new"
            })
            .to_string(),
        ))
        .unwrap();
        assert_eq!(editor.function.name, "str_replace");
        assert_eq!(json_args(&editor)["old_string"], "old");
        assert_eq!(json_args(&editor)["new_string"], "new");
    }

    #[test]
    fn translates_all_editor_commands() {
        let temp = tempfile::tempdir().unwrap();
        let existing = temp.path().join("existing.txt");
        std::fs::write(&existing, "one\ntwo\n").unwrap();
        let missing = temp.path().join("missing.txt");

        let view = resolve_execution_call(&editor_call(json!({
            "command": "view",
            "path": existing,
            "view_range": [2, -1]
        })))
        .unwrap();
        assert_eq!(view.function.name, "read_file");
        assert_eq!(json_args(&view)["offset"], 2);

        let create = resolve_execution_call(&editor_call(json!({
            "command": "create",
            "path": missing,
            "file_text": "new"
        })))
        .unwrap();
        assert_eq!(create.function.name, "write_file");
        assert_eq!(json_args(&create)["content"], "new");

        let insert = resolve_execution_call(&editor_call(json!({
            "command": "insert",
            "path": existing,
            "insert_line": 1,
            "new_str": "middle"
        })))
        .unwrap();
        assert_eq!(insert.function.name, "str_replace");
        assert_eq!(json_args(&insert)["new_string"], "one\nmiddle\ntwo\n");
    }

    #[test]
    fn rejects_relative_editor_paths_and_existing_create_targets() {
        let relative = resolve_execution_call(&editor_call(json!({
            "command": "view",
            "path": "src/main.rs"
        })))
        .unwrap_err();
        assert!(relative.to_string().contains("not an absolute path"));

        let temp = tempfile::NamedTempFile::new().unwrap();
        let existing = resolve_execution_call(&editor_call(json!({
            "command": "create",
            "path": temp.path(),
            "file_text": "replacement"
        })))
        .unwrap_err();
        assert!(existing.to_string().contains("Cannot overwrite"));
    }

    /// 参数尾部多带内容时取第一个完整对象，schema 校验照常进行。
    #[test]
    fn tolerates_trailing_content_after_anchor_arguments() {
        let bash =
            resolve_execution_call(&call(BASH_NAME, r#"{"command":"pwd"} 残余片段"#)).unwrap();

        assert_eq!(bash.function.name, "run_command");
        assert_eq!(json_args(&bash), json!({"command": "pwd"}));
    }

    /// 非对象参数与说明文字在前时仍然拒绝，避免把示例参数当成真实调用。
    #[test]
    fn rejects_non_object_and_leading_prose_anchor_arguments() {
        let array = resolve_execution_call(&call(BASH_NAME, "[\"pwd\"] 残余片段")).unwrap_err();
        assert!(
            array
                .to_string()
                .contains("invalid DeepSeek anchor tool arguments"),
            "{array}"
        );

        let prose =
            resolve_execution_call(&call(BASH_NAME, "示例：\n{\"command\":\"pwd\"}")).unwrap_err();
        assert!(
            prose
                .to_string()
                .contains("invalid DeepSeek anchor tool arguments"),
            "{prose}"
        );
    }

    fn call(name: &str, arguments: &str) -> ToolCall {
        ToolCall {
            id: "call-1".to_string(),
            kind: "function".to_string(),
            function: ToolCallFunction {
                name: name.to_string(),
                arguments: arguments.to_string(),
            },
        }
    }

    fn editor_call(arguments: Value) -> ToolCall {
        call(EDITOR_NAME, &arguments.to_string())
    }

    fn json_args(call: &ToolCall) -> Value {
        serde_json::from_str(&call.function.arguments).unwrap()
    }
}
