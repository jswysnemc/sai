mod buffered;

pub(crate) use buffered::render_buffered_image;

include!("terminal_image/protocol.rs");
include!("terminal_image/escape.rs");
include!("terminal_image/renderers.rs");
include!("terminal_image/raster.rs");
include!("terminal_image/halfblock.rs");
include!("terminal_image/tests.rs");
