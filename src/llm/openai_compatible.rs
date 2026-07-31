mod provider_routing;

include!("openai_compatible/client.rs");
include!("openai_compatible/client_helpers.rs");
include!("openai_compatible/request.rs");
include!("openai_compatible/claude_style.rs");
include!("openai_compatible/stream_types.rs");
include!("openai_compatible/stream_handlers.rs");
include!("openai_compatible/text_filters.rs");
include!("openai_compatible/tests.rs");
include!("openai_compatible/stream_error_tests.rs");
include!("openai_compatible/tag_strip.rs");
