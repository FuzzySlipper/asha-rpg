//! Persistent authority session for compiled ASHA RPG artifacts.
//!
//! The session is the only owner of mutable RPG state. A command, its explicit
//! random evidence, and any typed reaction decision are resolved in one staged
//! transaction before state becomes observable.

#![forbid(unsafe_code)]

mod replay;
mod semantic_session;

pub use replay::*;

pub use semantic_session::{
    RpgAuthorityCommand, RpgAuthoritySession, RpgCommandOutcome, RpgPendingReaction,
    RpgReactionCommand,
};
