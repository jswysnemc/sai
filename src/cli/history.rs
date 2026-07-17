use super::*;

pub(super) fn run_history(paths: &SaiPaths, args: HistoryArgs) -> Result<()> {
    let state = StateStore::new(paths)?;
    for entry in state.history(args.limit)? {
        if args.raw {
            println!("{}", serde_json::to_string(&entry)?);
            continue;
        }
        println!("{} {}", entry.timestamp, entry.role);
        if entry.role == "assistant" {
            let response = crate::llm::ChatResult {
                content: entry.content,
                reasoning: if args.no_thinking {
                    None
                } else {
                    entry.reasoning
                },
                usage: None,
                tool_calls: Vec::new(),
            };
            render::print_assistant_response(&response, !args.no_thinking)?;
        } else {
            println!("{}", entry.content);
        }
        println!();
    }
    Ok(())
}
