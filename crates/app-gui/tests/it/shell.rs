//! Headless frames of the shell.

use crate::support::{gui, synced_state};

#[test]
fn an_idle_frame_issues_no_commands() {
    let mut h = gui(synced_state());
    h.run();
    assert!(h.state_mut().take().is_empty());
    h.run();
    assert!(h.state().out.is_empty());
}
