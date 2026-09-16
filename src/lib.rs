//! Harness-agnostic core for devflow.
//!
//! The deterministic half of the design in `docs/design.md`: on-disk state, the work
//! graph, and the git operations around Executor clones. Agent harnesses (a Claude Code
//! skill today, something else later) drive this core; it never spawns or talks to agents
//! itself.

pub mod state;

pub use state::{ExecutorState, NodeState, NodeStatus, Phase, State};
