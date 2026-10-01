#[cfg(test)]
mod tests {
    include!("tests_core.rs");
    include!("tests_stream_protocol.rs");
    include!("tests_stream_buffer.rs");
    include!("tests_deepseek.rs");
    include!("tests_anthropic.rs");
    include!("tests_assistant_roundtrip.rs");
}
