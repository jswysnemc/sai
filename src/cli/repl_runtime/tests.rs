use super::reflow_state::ReflowState;
use super::viewport::TerminalSize;

#[test]
fn resize_during_stream_requires_finish_reflow() {
    let mut state = ReflowState::new();
    state.observe(TerminalSize { cols: 80, rows: 24 }, false);
    state.observe(
        TerminalSize {
            cols: 100,
            rows: 24,
        },
        true,
    );

    assert!(state.take_stream_finish_reflow_needed());
    assert!(!state.take_stream_finish_reflow_needed());
}
