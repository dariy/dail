//! Integration tests of `app-core`, built as one binary: a crate links one test executable
//! instead of one per file.

mod support;

mod accounts;
mod agents;
mod approvals;
mod attachments;
mod backfill;
mod bodies;
mod checked;
mod condstore;
mod contacts;
mod discovery;
mod draft_saves;
mod drafts;
mod engine;
mod envelopes;
mod expanded;
mod flags;
mod history;
mod identities;
mod images;
mod keys;
mod mailboxes;
mod moves;
mod panes;
mod pending;
mod preview;
mod registry;
mod replay;
mod saves;
mod scale;
mod search;
mod searching;
mod sends;
mod sync;
mod threading;
mod undo;
mod unified;
mod view;
mod watch;
