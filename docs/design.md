# devflow — Orchestrated Development Flow

A two-role agent architecture for taking a request from intent to landed code, with
durable state, explicit human gates, and parallel execution of independent work.

Status: **design, not implemented.** This document defines the architecture and the
contracts between components. Implementation form (Claude Code skill + subagents vs.
a standalone harness) is deliberately deferred; see [Open Questions](#12-open-questions).

---

## 1. Purpose

Long agent sessions fail in predictable ways: context is rebuilt from scratch on every
resume, the human approves a direction and then watches the agent drift from it, and a
single sprawling diff arrives at the end that nobody can meaningfully review.

devflow addresses those three failures with three mechanisms:

| Failure | Mechanism |
|---|---|
| Context evaporates between sessions | Every phase writes a durable artifact to disk |
| Agent drifts from the agreed direction | Every phase transition waits on explicit human approval |
| One unreviewable diff at the end | Work is decomposed into a graph of independently reviewable nodes |

This design complements, and does not replace,
[`implementation.md`](../../ai-artifacts/instructions/implementation.md). That document
already defines how a single code change must be made — scope discipline, tests within
scope, self-review, human approval on the full diff, and the human performing the commit.
devflow is the *scheduler* around that document: an Executor's lifecycle is
`implementation.md` turned into a state machine, and the Orchestrator decides which
changes exist and in what order.

---

## 2. Roles

### Orchestrator

One per project. Long-lived. The only agent that talks to the human.

Owns: the goal, context, design, the work graph, scheduling, and run state.

**Does not write production code or git history.** Its only writes are the project
artifacts (§9), `.devflow/state.json`, each node's `work_item.md`, provisioning Executor
directories (cloning a codebase in, removing it once landed), and refreshing its own
codebase copies. This is the central invariant — it keeps the Orchestrator's context
budget spent on coordination rather than implementation detail, and it means every line
of production code has passed through a node's review and confirm gates.

Its copies of the codebases are **read-only**. They change only when the Orchestrator
refreshes them to load changes the human has landed (§8.3).

### Executor

One per work node. Ephemeral, headless, no channel to the human. Spawned by the
Orchestrator with the node's work item; works in its own copy of exactly one codebase (§8.1).

Owns: the implementation of exactly one node, its tests, and its self-review.

**Does not commit, re-plan, or touch other nodes.** Its changes stay uncommitted in its
copy for the human to review and land. An Executor that concludes its work item is wrong
escalates to the Orchestrator rather than improvising a better plan.

Its only message to the Orchestrator is its completion status — ready for confirm, or
blocked (§7.1). It has no other channel and writes no artifact outside its clone.

### Human

Initializes the project: creates the project repository and populates the codebases
(§9). Approves the goal, the design, and the plan. Reviews each node's diff at its
confirm gate.

**Performs every git history operation** — committing the Orchestrator's artifacts in the
project repository, and committing, landing, and resolving conflicts for node changes in
the codebases.

---

## 3. Invariants

These are the properties a correct implementation must preserve. Most of the design
below follows from them.

- **I1.** The Orchestrator never edits production code.
- **I2.** Exactly one writer per file. `state.json` and `work_item.md` are
  Orchestrator-only; an Executor writes only inside its own clone. No locking is required
  because no file has two writers.
- **I3.** No phase advances without explicit human approval. The phase itself is the
  record: the Orchestrator never moves on from a gate it has not been told to pass.
- **I4.** An Executor owns exactly one node and one Executor directory, and edits nothing
  outside it.
- **I5.** State is durable *before* the action it authorizes (write-ahead). A crash
  between "wrote intent to spawn" and "spawned" is recoverable; the reverse is not.
- **I6.** An Executor that finds its work item wrong escalates. Re-planning is an
  Orchestrator action and requires a fresh human approval.
- **I7.** Only the human writes git history, in the project repository and in every
  codebase. No agent commits, merges, rebases, or pushes.
- **I8.** The Orchestrator's codebase copies are read-only. They change only by a
  fast-forward refresh to changes the human has already landed.

---

## 4. Run Lifecycle

A **run** is one pass through this lifecycle, from intake to `done`, for one approved
goal. It has one goal, design, and work graph, and one `state.json`. Re-planning and design revisions (§11) happen *within* a run — they amend it
rather than start a new one. A project has exactly one run (§12).

```mermaid
stateDiagram-v2
    [*] --> intake
    intake --> exploring: goal approved
    exploring --> designing
    designing --> planning: design approved
    planning --> executing: plan approved
    executing --> planning: work item wrong
    executing --> designing: design wrong
    executing --> done: all nodes landed
    done --> [*]
```

Each gated phase (intake, design, planning) iterates with the human until approval; those
loops are omitted from the diagram.

### 4.1 Intake

The human describes what needs to be done. The Orchestrator restates it as `goal.md` and
iterates with the human until they agree. Cheap, and it catches the most expensive class
of error — building the wrong thing — before any tokens are spent on exploration.

`goal.md` states:

- the request, in the Orchestrator's own words
- what "done" looks like, concretely enough to check at the end of the run
- known constraints (compatibility, systems that must not change, deadlines)
- what is explicitly out of scope

**Gate: the human approves `goal.md` explicitly.** The phase advances to `exploring` only
on that approval; exploration does not begin until then.

Output: `goal.md`.

### 4.2 Exploration

The Orchestrator reads the relevant codebases from its read-only copies and writes down
what it learned. Fan-out search agents are appropriate here; their findings are distilled
into the document, not pasted into it.

Exploration is scoped by the approved goal, not exhaustive. The aim is enough context to
design, not a map of every codebase.

Output: `understanding.md` — the codebases available and what each is, architecture as
it actually is, relevant conventions, constraints discovered, and **open questions**.
Open questions are first-class; an unanswered one is a reason to talk to the human, not
to guess.

### 4.3 Design

The Orchestrator proposes a high-level design and iterates with the human. The design
answers *what* changes and *why*, and explicitly names what is out of scope.

A design is complete when it states: the approach, the alternatives rejected and why,
the components affected, the new or changed interfaces, and the non-goals.

**Gate: the human approves `design.md` explicitly.** The phase advances to `planning` only
on that approval (I3).

Output: `design.md`.

### 4.4 Planning

The Orchestrator decomposes the approved design into a directed acyclic graph of work
nodes. Node contract and sizing rules are in §5.

The plan is presented to the human in two forms: `plans.md` for reading, `graph.json` for
machines. They must agree; `plans.md` is generated from `graph.json`.

**Gate: the human approves the plan explicitly.** The phase advances to `executing` only
on that approval.

Output: `plans.md`, `graph.json`.

### 4.5 Execution

The Orchestrator schedules ready nodes, spawns Executors, relays confirm gates to the
human, refreshes its codebase copies as the human lands work, and persists state after
every transition. Detail in §6–§8.

### 4.6 Interaction Flow

End-to-end interactions across every participant in a run.

| Participant | What it is |
|---|---|
| Human | Approves gates, reviews diffs, performs all git history operations |
| Orchestrator | Coordinating agent; sole channel to the human |
| Explorers | Short-lived search agents fanned out during exploration |
| Project repo | The project directory; checked-in artifacts (§9) |
| `state.json` | Run state, in `.devflow/` |
| Codebase copies | The Orchestrator's read-only copies, in `.devflow/repos/` |
| Executor | Per-node implementing agent |
| Executor dir | The node's work item and codebase clone, in `.devflow/executors/` |
| Upstream | Wherever the human lands codebase changes; the codebase copies' remotes |

```mermaid
sequenceDiagram
    actor H as Human
    participant O as Orchestrator
    participant X as Explorers
    participant P as Project repo
    participant S as state.json
    participant C as Codebase copies
    participant E as Executor
    participant W as Executor dir
    participant U as Upstream

    Note over H,U: Intake (§4.1). The human commits artifacts in P whenever they choose.
    H->>O: request
    loop until approved
        O->>P: write goal.md
        O->>H: present goal.md
        H-->>O: feedback or approval
    end
    O->>S: phase exploring

    Note over H,U: Exploration (§4.2)
    O->>X: scoped searches
    X->>C: read code
    X-->>O: findings
    O->>P: write understanding.md
    opt open questions
        O->>H: questions
        H-->>O: answers
    end

    Note over H,U: Design (§4.3)
    loop until approved
        O->>P: write design.md
        O->>H: present design.md
        H-->>O: feedback or approval
    end
    O->>S: phase planning

    Note over H,U: Planning (§4.4)
    loop until approved
        O->>P: write graph.json and plans.md
        O->>H: present plan
        H-->>O: feedback or approval
    end
    O->>S: phase executing, node statuses

    Note over H,U: Execution (§4.5), nodes run concurrently up to the limit
    loop each ready node
        O->>S: status running, executor working
        O->>W: write work_item.md, clone codebase from C
        O->>E: spawn with work item
        E->>W: implement, test, self-review
        E-->>O: message: ready for confirm, test results
        O->>S: executor done
        O->>H: present diff and completion note
        alt approve
            H->>W: commit
            H->>U: land changes, resolve conflicts
            H-->>O: approve, already landed
            O->>C: refresh from U, fast-forward only
            O->>S: status complete
            O->>W: remove codebase clone
        else revise
            H-->>O: feedback
            O->>S: executor working
            O->>E: resume with feedback
        else reject
            H-->>O: reject
            O->>S: status abandoned
            O->>H: propose re-plan (§11)
        end
    end
    O->>H: run complete
```

---

## 5. The Work Graph

### 5.1 Node contract

Each node declares:

| Field | Purpose |
|---|---|
| `id` | Stable identifier; also the Executor directory name |
| `repo` | The one codebase this node changes |
| `title` | One line, imperative |
| `intent` | What changes and why, in prose. The core of the Executor's work item. |
| `depends_on` | Node ids that must be landed and refreshed before this one starts |
| `files_touched` | Predicted paths/globs within `repo`. Used for sibling scheduling (§8.2), not enforcement. |
| `acceptance` | Verifiable criteria. "The endpoint returns 409 on duplicate email", not "auth works". |
| `tests` | What tests this node must add or change, and how to run them |
| `gate` | `manual` (default) or `auto` — see §12 |

### 5.2 Sizing rules

A node must be **reviewable, verifiable, and incremental**. Concretely:

- **Reviewable** — one human can hold the whole diff in their head. Heuristic: ≲400
  changed lines across ≲5 files. Past that, split.
- **Verifiable** — the acceptance criteria can be checked by running something. A node
  whose criteria can only be assessed by opinion is under-specified.
- **Incremental** — the codebase is in a working state after the node lands. A node
  that leaves the build broken pending a sibling is not a node; it and its sibling are
  one node.
- **Single-repo** — a node changes exactly one codebase. A change that spans codebases is
  several nodes linked by `depends_on`.

**Splitting test:** if the node's title needs the word "and", or a reviewer would have to
mentally sort the diff into categories, split it. This is the same heuristic
`implementation.md` applies to a logical scope, because a node *is* a logical scope.

### 5.3 Graph rules

- Acyclic, validated at plan approval.
- `depends_on` means "must be **landed and refreshed** into the Orchestrator's copy
  before this starts", not "must be complete". The distinction matters: a dependent
  node's codebase is cloned from that refreshed copy, so it already contains its
  dependencies and never re-does or conflicts with them.
- Nodes with no unlanded dependencies are `ready`.

---

## 6. Executor Lifecycle

```mermaid
stateDiagram-v2
    direction LR
    [*] --> implementation
    implementation --> test
    test --> review
    review --> confirm
    confirm --> complete
    complete --> [*]
```

The diagram shows the path to completion. Off that path:

| From | Back to `implementation` when | Budget | When exhausted |
|---|---|---|---|
| test | tests fail | 3 | `blocked` |
| review | self-review has findings | 2 | `blocked` |
| confirm | the human requests revisions | none | — |

An Executor also ends `blocked` if it finds its work item wrong (I6), and `abandoned` if
the human rejects the node outright at confirm.

### implementation
Make the change described by the work item, plus the tests that verify it. Scope is the node
and nothing else — discoveries outside it are reported, not fixed (`implementation.md` §1).

### test
Run the node's tests plus any broader suite reasonably affected. All must pass. A failing
test is investigated, never disabled or weakened.

### review
Self-review the full diff against correctness, scope discipline, readability, test quality,
leftovers, and consistency (`implementation.md` §4). Findings are fixed within scope;
findings that require leaving scope are escalated.

### confirm
The Executor **stops** and messages the Orchestrator that it is done. It cannot reach the
human itself — the Orchestrator relays (§7). On revision feedback it re-enters `implementation` with that
feedback, as many times as the human wants; on outright rejection the node is `abandoned`
and the Orchestrator escalates to re-planning.

### complete
The human landed the changes and approved at confirm (§8.3). The Executor is finished.

---

## 7. The Confirm Relay

### 7.1 Mechanism

```mermaid
sequenceDiagram
    actor H as Human
    participant O as Orchestrator
    participant E5 as Executor n5
    participant E6 as Executor n6

    E5-->>O: done, ready for confirm
    O->>H: present n5 diff and completion note
    O->>E6: dispatch n6 into the free slot
    Note over H,E6: human reviews n5 while n6 runs
    alt approve
        H-->>O: approve n5, already landed
        Note over O,E5: n5 Executor released, Orchestrator refreshes (§8.3)
    else revise
        H-->>O: feedback on n5
        O->>E5: resume in place with feedback
        E5-->>O: done again, ready for confirm
    else reject
        H-->>O: reject n5
        Note over O,E5: n5 abandoned, re-plan (§11)
    end
```

**How the Executor signals.** It messages the Orchestrator with its completion status:
the outcome (`confirm` or `blocked`) and a **completion note** — a few lines covering the
tests it ran and their result, plus anything the human must know to review the diff. That
is the Executor's only channel and its only output besides the code in its clone. It writes
no durable artifact of its own.

The Orchestrator records `executor: done` in `state.json`, then presents the diff and the
note to the human. The diff is read from the clone, so the Orchestrator's context is not
spent on implementation detail (§2). The note itself is not persisted — after a restart the
diff is re-presented without it (§10).

Messages are for completion status only — no progress updates while working. So the
Orchestrator cannot see inside a running Executor, which is why it tracks only `working` or
`done` rather than the Executor's internal phases (§6). It also means a crash loses nothing
that has to be recovered: a respawned Executor reads its clone and picks up from the code
as it stands (§10).

On revision the Executor is resumed in place, retaining its context. Restarting it from
cold would discard everything it learned implementing the node — the reason the relay
resumes rather than respawns.

While `n5` awaits confirm, the Orchestrator **keeps scheduling other ready nodes** up to
the concurrency limit. Human review time overlaps with machine work rather than blocking
it. Dependents of `n5` stay `pending`, since `depends_on` requires landing.

### 7.2 What gets approved

The gate exists so that **no node finishes without explicit human approval**. The Executor
stops and waits for it; the Orchestrator does not decide on the human's behalf. Since the
human lands the change first (§8.3), the approval is the record that they reviewed it, not
a lock on code they have not seen.

The human reviews a specific diff: the codebase clone's working tree against the base
commit it was cloned at, including untracked files. Taking it against the base commit
rather than the clone's HEAD means the human committing the changes does not alter it.

**The human lands the changes before approving** (§8.3). An approval therefore means both
"this diff is accepted" and "this diff has landed" — there is no separate state for
approved but not yet landed, and nothing to bind the approval to, since the human put the
code there themselves. Edits they make while landing, such as conflict resolution, are
theirs; the landing check in §8.3 reports what actually landed.

Because dependents wait on landing, the human's review-and-land time gates progress
through the graph.

---

## 8. Isolation and Integration

### 8.1 Codebase copies

The Orchestrator reads from its own copy of each codebase at `.devflow/repos/<repo>/`.
Each node gets an Executor directory holding its work item and a clone of the node's one
codebase:

```
.devflow/
├── repos/
│   └── <repo>/            # Orchestrator's read-only copy
└── executors/
    └── <node-id>/
        ├── work_item.md
        └── <repo>/        # clone of repos/<repo> at dispatch
```

The clone is taken from the Orchestrator's copy **at dispatch time**. Because dependencies
are landed and refreshed before dependents dispatch, a dependent's clone already contains
its dependencies' work.

The clone is a local `git clone`, not a worktree. A worktree writes into the source
repository's `.git` on creation and on every commit made in it, which would break the
read-only copy (I8). A local clone on the same filesystem hardlinks the object store, so
it is nearly as cheap and never writes to the source.

The work item sits *beside* the clone, not inside it, so it never appears as an untracked
file in the diff under review. A node has one Executor at a time; an Executor respawned
after a crash reuses the directory and its partial work (§10).

Separate clones buy three things: concurrent edits without corruption, independent test
runs (no shared build lock or port collision), and a clean per-node diff for review.

They cost duplicated build state. `node_modules`, virtualenvs, and build caches are not
shared across clones. Mitigations, in order of preference: a shared package store
(pnpm, uv), symlinking the dependency directory into each clone at setup, or accepting
the install cost. **This is the design's main operational tax and should be measured
before committing to it** — on a codebase with a 4-minute cold install, three parallel
nodes cost 12 minutes of setup to save perhaps 20 of execution.

### 8.2 Sibling scheduling

Separate clones prevent *corruption* between parallel nodes but not *conflicts* when their
changes land. The Orchestrator therefore co-schedules siblings in the same codebase only
when their predicted `files_touched` sets are disjoint. Overlapping siblings are
serialized. Siblings in different codebases never overlap.

`files_touched` is a prediction and will sometimes be wrong. It is a scheduling heuristic,
not an enforcement boundary — a wrong prediction degrades to a conflict the human
resolves while landing (§8.3), not to corrupted state.

Concurrency limit: default 3 in-flight Executors. The binding constraint is usually the
human's review and landing throughput, not the machine's.

### 8.3 Landing and refresh

Before approving, the human **lands** the node: commits the changes in the Executor's
clone and delivers them to the branch the Orchestrator's copy tracks, by whatever git flow
they choose, resolving any conflicts along the way. devflow does not commit, merge,
rebase, or push (I7).

The approval tells the Orchestrator the node has landed. The Orchestrator:

1. Refreshes its copy of that codebase with a fast-forward-only pull. If the copy cannot
   fast-forward, it stops and surfaces the problem; it never resolves divergence itself.
2. Checks that the approved diff is present — it reverse-applies cleanly to the refreshed
   copy (`git apply --reverse --check`). This holds for squash merges and cherry-picks as
   well as plain merges. If the check fails, for example because the human changed the
   code while resolving a conflict, the Orchestrator asks the human to confirm the node
   landed.
3. Marks the node `complete`, removes the codebase clone from the Executor directory
   (keeping `work_item.md`), and recomputes which nodes are `ready`.

---

## 9. On-Disk Layout

A devflow project is a git-initialized directory that represents the project being
implemented. It is the directory the Orchestrator is started in. The Orchestrator's
artifacts live at its root and are committed by the human; everything operational lives
in `.devflow/`, which is never checked in.

```
<project>/                      # project repo: single branch, human commits
├── .gitignore                  # ignores .devflow/
├── goal.md                     # approved goal
├── understanding.md            # codebases, exploration context, open questions
├── design.md                   # approved high-level design
├── plans.md                    # human-readable execution plan (generated from graph.json)
├── graph.json                  # the DAG — machine-readable
└── .devflow/                   # temp, not checked in
    ├── state.json              # run phase + node statuses  (Orchestrator-owned)
    ├── repos/
    │   └── <repo>/             # Orchestrator's read-only codebase copy
    └── executors/
        └── <node-id>/
            ├── work_item.md    # the node contract handed to the Executor  (Orchestrator-owned)
            └── <repo>/         # the Executor's codebase clone
```

Schemas for `graph.json` and `state.json`: [`state-schema.md`](./state-schema.md).

**Initialization.** The human creates the project repository and populates
`.devflow/repos/` with every codebase the project needs — directly, or from a list of
codebases checked in to the project repository. devflow does not choose or fetch
codebases on its own.

`understanding.md` is **append-only with dated entries**. Context discovered during
exploration is often invalidated during execution. Corrections reach the Orchestrator from
the human, or from its own reading of a refreshed codebase copy — not from Executors,
which only report completion (§7.1). The Orchestrator appends them rather than rewriting
history, so a resumed run can see that a belief changed and when.

Because `.devflow/` is not checked in, **execution state is local to one machine**. The
checked-in artifacts travel with the project repository; `state.json`, work items, and
in-progress clones do not. See §12.

---

## 10. Resumability

The Orchestrator is stateless between sessions; `state.json` is the sole source of truth.
On start it reads the project and `.devflow/` and reconstructs.

**A subagent that was in flight when the session died is gone, but its clone survives**, and
the clone is the work. Nothing has to be reconstructed: a replacement Executor reads the
clone to see how far things got and continues from there. It loses the dead Executor's
reasoning, not its output, so recovery can be blunt.

This is why `state.json` holds so little (§9). Which Executor is working which node is
bookkeeping the Orchestrator keeps in session: those handles are dead after a restart
anyway, so persisting them would buy nothing.

Recovery protocol, per node:

| Recorded state | Action |
|---|---|
| `pending`, `ready` | Nothing was in flight. Schedule normally. |
| `running`, executor `working` | Respawn an Executor in the same directory with the work item, plus the human's feedback if it was mid-revision. |
| `running`, executor `done` | Re-present the clone's current diff. If the human had already landed it before the crash, they say so and the Orchestrator continues with §8.3. |
| `complete` | Remove the clone if it is still there. |
| `blocked`, `abandoned` | Surface to the human; do not auto-retry. |

Writes to `state.json` are atomic (write to a temp file, `fsync`, `rename`). Combined with
write-ahead ordering (I5), a crash at any point leaves state that is either correct or
conservatively stale — never ahead of reality. Stale is cheap here: the worst case is an
Executor re-doing work that is already sitting in its clone.

---

## 11. Failure Handling

**Node fails permanently (`blocked`).** Dependents become `blocked` transitively.
Independent branches of the graph keep running. The human is told which subtree stalled
and why.

**Work item is wrong.** The Executor escalates with what it found (I6). The Orchestrator
proposes a graph amendment. **A plan amendment requires fresh human approval** — otherwise
the approved plan and the executing plan drift apart, which is precisely the failure this
design exists to prevent.

**Design is wrong.** Rare and expensive. The run returns to `designing`. Landed nodes stay
landed; the amended plan accounts for them as existing state.

**Conflict at landing.** Resolved by the human (§8.3). devflow neither rebases the node nor
re-opens its confirm gate.

**Refresh cannot fast-forward.** The Orchestrator's copy has diverged from its upstream.
Surfaced to the human; the Orchestrator does not reset or merge its copy (I8).

---

## 12. Open Questions

1. **Implementation form.** Claude Code skill + subagents (cheap, works today, constrained
   by what the Agent tool exposes) vs. a standalone harness on the Agent SDK (full control
   over scheduling, resume, and approval UX; a runtime to maintain). Deferred.

2. **`gate: auto` nodes.** The node contract reserves the field but this design treats
   every node as `manual`. Auto-gating low-risk nodes (config, mechanical renames) on
   green tests plus clean review would remove the human from the hot path where the risk
   doesn't justify them. Needs a rule for what qualifies, and it should not be the
   planning agent's unchecked judgment.

3. **The human is the intended bottleneck, by design.** Once the goal, design, and plan are
   approved, the human's remaining job is review. A 20-node run means 20 diffs reviewed and
   landed, and that is the trade being made, not a flaw to engineer around. If it becomes
   painful, the levers are batched review of independent completed nodes and, eventually,
   removing the human from low-risk nodes (item 2). Neither is designed here.

4. **Parallelism is deliberately modest.** Two or three in-flight Executors is enough;
   serializing siblings that overlap is an acceptable fallback (§8.2). Clone setup cost
   (§8.1) only needs to stay below the benefit at that small scale.

5. **Execution state is machine-local** (§9). The artifacts are checked in, but
   `.devflow/` is not, so a run cannot resume execution from a different checkout.

6. **Nested orchestration** — whether an Executor may ever spawn sub-Executors — is
   deliberately excluded. It breaks I4 and makes the state machine substantially harder
   to reason about. Revisit only with evidence of need.

7. **Human-owned git (I7) is a starting policy.** It keeps agents out of history but puts
   commits, landing, and conflict resolution on the human's hot path. Open: whether to
   later let agents land clean, approved nodes, and whether anything should re-run a
   node's tests after the human resolves a conflict while landing.

8. **One run per project.** A project with a follow-up goal after `done` would need run
   identifiers in the layout (§9) and in `state.json`. Not designed until needed.

9. **Hash-bound approvals were dropped for the MVP.** Earlier drafts recorded every
   approval against a content hash, so editing an approved artifact voided it. `state.json`
   now holds only the phase and node statuses (§9). Artifact drift is still visible: the
   human commits `goal.md`, `design.md`, and `graph.json` to the project repository. Restore
   the ledger if drift turns out to happen in practice.

10. **Turning a design into a good graph is under-specified.** §5.2 gives sizing rules, but
   nothing guides decomposition itself, and the plan's quality determines everything
   downstream. Iterate on this after the MVP, with real graphs to learn from.
