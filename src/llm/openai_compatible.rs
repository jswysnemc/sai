mod anthropic_transport;
mod provider_routing;
mod stream_completion;
mod anthropic_content;

use stream_completion::require_completion;
use anthropic_content::{AnthropicContentAccumulator, apply_default_cache_control};

#[cfg(test)]
mod tests_transport_lifecycle;

include!("openai_compatible/client.rs");
include!("openai_compatible/client_anthropic.rs");
include!("openai_compatible/client_helpers.rs");
include!("openai_compatible/request.rs");
include!("openai_compatible/claude_style.rs");
include!("openai_compatible/stream_types.rs");
include!("openai_compatible/stream_handlers.rs");
include!("openai_compatible/text_filters.rs");
include!("openai_compatible/tests.rs");
include!("openai_compatible/stream_error_tests.rs");
include!("openai_compatible/tag_strip.rs");
