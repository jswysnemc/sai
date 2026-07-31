mod colors;
mod model;
mod renderer;

#[cfg(test)]
mod tests;

pub(crate) use renderer::{
    diff_body_start_column, render_edit_file_diff, render_edit_file_diff_for_transcript,
    write_edit_file_diff_block,
};
