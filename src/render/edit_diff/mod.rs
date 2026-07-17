mod colors;
mod model;
mod renderer;

#[cfg(test)]
mod tests;

pub(crate) use renderer::{render_edit_file_diff, write_edit_file_diff_block};
