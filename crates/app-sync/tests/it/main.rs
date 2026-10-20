//! Integration tests of `app-sync`, built as one binary: a crate links one test executable
//! instead of one per file.

mod support;

mod audit;
mod ipc;
mod live;
mod live_it;
mod saves;
mod session;
