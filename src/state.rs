//! `state.json` — the run's phase and its node statuses, owned by the Orchestrator.
//!
//! Field-for-field, this is the schema in `docs/state-schema.md` §2. It holds only what
//! cannot be derived or re-observed: everything else, including which Executor is working
//! which node, is session bookkeeping the Orchestrator keeps itself.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Run state. One per project, since a project has exactly one run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub phase: Phase,
    /// Keyed by node id, matching an id in `graph.json`. Empty until a plan is approved.
    #[serde(default)]
    pub nodes: BTreeMap<String, NodeState>,
}

impl State {
    /// A fresh run, before intake has produced anything.
    pub fn new() -> Self {
        Self {
            phase: Phase::Intake,
            nodes: BTreeMap::new(),
        }
    }
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

/// Advances only on explicit human approval (invariant I3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Intake,
    Exploring,
    Designing,
    Planning,
    Executing,
    Done,
}

/// The Orchestrator's scheduling view of one node.
///
/// Built only through the constructors below, so `executor` is set exactly when the node
/// is `running` and never otherwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeState {
    pub status: NodeStatus,
    /// Non-null only while `status` is `running`.
    pub executor: Option<ExecutorState>,
}

impl NodeState {
    /// Waiting on dependencies that have not landed.
    pub fn pending() -> Self {
        Self::idle(NodeStatus::Pending)
    }

    /// Dependencies landed; eligible for dispatch.
    pub fn ready() -> Self {
        Self::idle(NodeStatus::Ready)
    }

    /// An Executor owns the node, either working or waiting on the human.
    pub fn running(executor: ExecutorState) -> Self {
        Self {
            status: NodeStatus::Running,
            executor: Some(executor),
        }
    }

    /// Landed, approved, and refreshed into the Orchestrator's copy.
    pub fn complete() -> Self {
        Self::idle(NodeStatus::Complete)
    }

    /// A budget was exhausted, or the Executor escalated.
    pub fn blocked() -> Self {
        Self::idle(NodeStatus::Blocked)
    }

    /// The human rejected the node outright.
    pub fn abandoned() -> Self {
        Self::idle(NodeStatus::Abandoned)
    }

    fn idle(status: NodeStatus) -> Self {
        Self {
            status,
            executor: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeStatus {
    Pending,
    Ready,
    Running,
    Complete,
    Blocked,
    Abandoned,
}

/// What the Orchestrator can observe of an Executor: it is working, or it has returned
/// with a completion status (design.md §7.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutorState {
    Working,
    Done,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_run_starts_at_intake_with_no_nodes() {
        let state = State::new();

        assert_eq!(state.phase, Phase::Intake);
        assert!(state.nodes.is_empty());
    }

    #[test]
    fn enums_serialize_as_the_documented_strings() {
        assert_eq!(serde_json::to_value(Phase::Executing).unwrap(), "executing");
        assert_eq!(
            serde_json::to_value(NodeStatus::Abandoned).unwrap(),
            "abandoned"
        );
        assert_eq!(
            serde_json::to_value(ExecutorState::Working).unwrap(),
            "working"
        );
    }

    #[test]
    fn a_node_without_an_executor_serializes_it_as_null() {
        let node = NodeState::pending();

        let json = serde_json::to_value(node).unwrap();

        assert_eq!(json["status"], "pending");
        assert!(json["executor"].is_null());
    }

    #[test]
    fn only_a_running_node_carries_an_executor() {
        let idle = [
            NodeState::pending(),
            NodeState::ready(),
            NodeState::complete(),
            NodeState::blocked(),
            NodeState::abandoned(),
        ];

        for node in idle {
            assert_ne!(node.status, NodeStatus::Running);
            assert!(node.executor.is_none(), "{:?} carries an executor", node);
        }
    }

    #[test]
    fn an_unknown_field_is_rejected_rather_than_dropped() {
        let json = r#"{"phase": "intake", "nodes": {}, "concurrency_limit": 3}"#;

        assert!(serde_json::from_str::<State>(json).is_err());
    }

    #[test]
    fn a_running_node_records_what_its_executor_is_doing() {
        let node = NodeState::running(ExecutorState::Done);

        let json = serde_json::to_value(node).unwrap();

        assert_eq!(json["status"], "running");
        assert_eq!(json["executor"], "done");
    }

    #[test]
    fn nodes_default_to_empty_when_absent() {
        let state: State = serde_json::from_str(r#"{"phase": "intake"}"#).unwrap();

        assert!(state.nodes.is_empty());
    }

    #[test]
    fn state_round_trips_through_json() {
        let mut state = State::new();
        state.phase = Phase::Executing;
        state
            .nodes
            .insert("n3".into(), NodeState::running(ExecutorState::Working));
        state.nodes.insert("n8".into(), NodeState::blocked());

        let json = serde_json::to_string(&state).unwrap();

        assert_eq!(serde_json::from_str::<State>(&json).unwrap(), state);
    }
}
