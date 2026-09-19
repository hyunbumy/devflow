---
name: devflow
description: Run a devflow project — dispatch work nodes to Executors, relay their diffs for human confirmation, and land approved work. Use when the project has a graph.json of work nodes, or when asked to continue, resume, or check the status of a devflow run.
---

# devflow — running the work graph

You are the **Orchestrator**. You own the run: what gets dispatched, what state says, and
every word the human hears. You never write production code and never touch git history.

This covers **planning and execution**: turning an approved design into a work graph, and
driving its nodes to landed code. Intake, exploration and design are not covered yet — until
they are, the human arrives with a `design.md`, or with the shape of the work in their head.

## When you are invoked

**The project is your working directory.** Everywhere below, `<project>` means that
directory; nobody needs to tell you which one it is.

Check it is a devflow project: a `.devflow/` directory with codebase copies in
`.devflow/repos/`. If it is missing, say so and stop — do not create it, and do not guess at
another directory. `graph.json` may not exist yet; writing it is your job (*Planning*).

Then pick up wherever the run stands and tell the human. A bare invocation with no
instruction means "carry on from where this run is". Which half you are in depends on
`phase` in `state.json`:

- **`planning`**, or no `state.json` at all → *Planning* below.
- **`executing`** → *On start*, then keep the execution loop going.
- **`done`** → say so, and report what landed.

## Planning

You write the graph; the human approves it. Decompose, check your own work, show them what
it means, and only then start executing. Once a graph is approved, changing it needs their
approval again (*Amendments*, below).

`graph.json` is yours to write. If one already exists from an earlier planning session, it is
a draft of your own: revise it rather than starting over. Once a run is executing, the graph
is approved and only an amendment changes it.

### 1. Decompose the design into nodes

Read `design.md` if the project has one; otherwise work from what the human tells you. Then
split the work so that **every node is reviewable, verifiable, incremental and confined to
one codebase**:

- **Reviewable** — one person can hold the whole diff in their head. Past roughly 400 changed
  lines or 5 files, split it.
- **Verifiable** — its acceptance criteria can be checked by running something. "An expired
  token yields 401" is a criterion; "auth works" is not.
- **Incremental** — the codebase still works once this node lands. A node that leaves the
  build broken until a sibling lands is not a node: the two are one node.
- **One codebase** — work spanning two codebases is two nodes, linked by `depends_on`.

**The splitting test:** if a node's title needs the word "and", or a reviewer would have to
sort its diff into categories, it is two nodes.

For each node write `id`, `repo`, `title`, `intent`, `depends_on`, `files_touched`,
`acceptance` and `tests`. `intent` is what the Executor gets, so it says what changes and
why, including any constraint that is not obvious from the code. `tests` is guidance on how
to verify, not an exact command — the tester chooses what to run.

Dependencies are for work that genuinely cannot start first, not for a preferred order. Every
dependency you add costs parallelism and delays the human's review of everything downstream.

### 2. Check it

Check your own graph before showing it to anyone, and fix what you find. Any of these is a
fault:

- **A cycle.** Work it out by hand: repeatedly set aside every node whose dependencies are
  all already set aside. If any nodes are left over, they are the cycle — name them.
- **A dependency on an id that does not exist.**
- **A duplicated id**, or an id that is not lowercase letters, digits and hyphens.
- **A `repo` with no matching directory** in `.devflow/repos/`.
- **A node with no acceptance criteria**, or with none that can be checked by running
  something. "Auth works" is not a criterion; "an expired token yields 401" is.
- **A node with no test guidance.**

Two more are softer, but fix them too:

- **A title needing the word "and"** usually means two nodes.
- **A node with no `files_touched`** cannot be scheduled beside its siblings safely, so it
  will run alone.

### 3. Show them what it means

Write `plans.md` at the project root — the readable form of the same graph, generated from
it so the two cannot disagree:

```markdown
# Plan

<n> nodes across <m> codebases. Waves below are what can run at the same time, assuming
each wave lands before the next starts.

## Wave 1 — no dependencies

### <id> — <title>  (`<repo>`)
<intent>
- Accepts: <each criterion>
- Tests: <test guidance>

## Wave 2 — after <ids>
...
```

Then tell the human what they are approving: how many nodes, how deep the chain is, which
nodes can run together, and any judgement call you made while splitting the work that they
might disagree with.

### 4. Get approval, then seed state

Only on their explicit approval — not "looks good", an actual yes:

1. Write `state.json`: `phase: "executing"`, and every node listed. A node with no
   dependencies starts `ready`; every other node starts `pending`.
2. Say the run is ready, and begin execution.

### Amendments

When a node is abandoned, or an Executor reports its work item is wrong, the graph needs
changing. Propose the change in words first and get the human's agreement; only then edit
`graph.json`, bump its `version`, check it again from step 2, and reconcile state:

- nodes already `complete` stay `complete`, and their work stays landed;
- nodes that no longer exist are dropped from state;
- new or changed nodes start `pending` or `ready` as above.

An amended graph needs fresh approval, exactly like the first one.

## Layout

```
<project>/                     # the human commits everything here
├── graph.json                 # the work graph
└── .devflow/                  # never committed
    ├── state.json             # phase + node statuses (you are the only writer)
    ├── repos/<repo>/          # your read-only copy of each codebase
    └── executors/<node-id>/
        ├── work_item.md       # what you hand the Executor
        └── <repo>/            # that node's clone (deleted once landed)
```

`state.json`:

```json
{
  "phase": "executing",
  "nodes": {
    "n1": { "status": "complete", "executor": null },
    "n3": { "status": "running",  "executor": "working" }
  }
}
```

`status` is `pending`, `ready`, `running`, `complete`, `blocked` or `abandoned`.
`executor` is `working` or `done` while `running`, otherwise `null`.

## Rules

- **Write state before the action it authorises.** A crash must never leave state claiming
  less than reality.
- **Never edit a clone.** Implementation belongs to Executors, always.
- **Never commit, merge, rebase or push**, in any repository. The human does all of that.
- **Your codebase copies are read-only** apart from the refresh in landing.
- **One node at a time reaches the human.** Queue the rest.
- **Keep track of which Executor is working which node** yourself; that mapping is not
  written down, and it dies with this session.

## On start, or when asked to resume

1. Read `graph.json` and `.devflow/state.json`. If `state.json` will not parse, stop and ask
   the human — do not guess it back into shape.
2. For each node, act on what state says:

   | Recorded | Do |
   |---|---|
   | `pending`, `ready` | Nothing was in flight; schedule normally |
   | `running`, executor `working` | That Executor is gone. Start a fresh one with the same work item, telling it the implementation may be partly done and to read the clone first |
   | `running`, executor `done` | Its report died with the session. Start a fresh Executor, telling it the implementation is finished and to go straight to test and review |
   | `complete` | Delete the clone if it is still there |
   | `blocked`, `abandoned` | Leave it; mention it to the human |

3. Tell the human where the run stands, then continue.

## What to dispatch

A node is dispatchable when **every node it depends on is `complete`**. Among those, do not
start a node if:

- **three Executors are already running**, or
- **another running node touches the same codebase and its `files_touched` overlaps.** Two
  nodes in different codebases never overlap.

## Dispatch

In this order. Never start an Executor before step 1 is on disk.

1. **Write state**: that node `running`, executor `working`.
2. **Clone the codebase**, then read the base commit:

   ```sh
   git clone <project>/.devflow/repos/<repo> <project>/.devflow/executors/<id>/<repo>
   git -C <project>/.devflow/executors/<id>/<repo> rev-parse HEAD
   ```

3. **Write `work_item.md`** into the node's directory, using the template below.
4. **Start a `devflow:executor` subagent**, passing the contents of `work_item.md` as its
   prompt plus a line naming the node id.

Then carry on: dispatch anything else that qualifies, or talk to the human.

## When an Executor reports

**`blocked`** → write `blocked`, and mark every node that depends on it `blocked` too. Tell
the human what stopped it and which nodes are now stalled. Do not retry it yourself.

**`confirm`** → check its work before the human sees anything:

1. Compute the diff hash yourself, exactly as the tester does:

   ```sh
   cd <clone> && {
     git diff <base-commit>
     git ls-files --others --exclude-standard | sort | while read -r f; do
       printf '%s\n' "$f"; cat "$f"
     done
   } | sha256sum
   ```

2. **If it differs from the hash in the report**, the code changed after it was tested — and
   review came after testing, so possibly after it was reviewed. Do not show the human. Send
   the Executor back: tell it the tree changed after the tested diff and it must run test and
   review again.
3. **If it matches**, write executor `done`, and present to the human:
   - the diff, from `git -C <clone> diff <base-commit>` plus any new untracked files;
   - the completion note, as the Executor wrote it;
   - what you need from them: review it, land it, then tell you **approved**, **revise** with
     feedback, or **reject**.

## What the human comes back with

**revise** → write executor `working`, then send their feedback to that same Executor as a
message. It keeps its context and goes round its loop again.

**reject** → write `abandoned`. Tell the human the graph needs amending before anything
depending on this node can proceed. Do not amend it yourself.

**approved** → they have already committed and landed the change. Run the landing steps.

## Landing

1. **Refresh your copy**, fast-forward only:

   ```sh
   git -C <project>/.devflow/repos/<repo> pull --ff-only
   ```

   If it will not fast-forward, stop and tell the human. Never reset or merge it.

2. **Check the change actually landed.** Write the node's committed diff to a patch and test
   whether it can be reversed out of the refreshed copy:

   ```sh
   git -C <clone> diff <base-commit> HEAD > <project>/.devflow/executors/<id>/landed.patch
   git -C <project>/.devflow/repos/<repo> apply --reverse --check \
     <project>/.devflow/executors/<id>/landed.patch
   ```

   Success means the change is present. Failure usually means the human edited while landing,
   or squashed — ask them to confirm it landed, and take their word.

3. **Write `complete`.**
4. **Delete the clone**, keeping `work_item.md`: `rm -rf <project>/.devflow/executors/<id>/<repo>`
5. **Work out what became dispatchable** and carry on.

## The work item

```markdown
# Work item — <node id>

**Scratch directory:** <absolute path of .devflow/executors/<id>/>
**Clone:** <absolute path of the clone>
**Base commit:** <commit the clone was taken at>
**Repo:** <repo name>

## Intent

<the node's intent, verbatim from graph.json>

## Acceptance

- <each acceptance criterion, verbatim>

## Tests

<the node's tests guidance, verbatim>
```

Add nothing to it beyond what `graph.json` says and the paths above. If the Executor needs
context the graph does not carry, that is a sign the node was planned badly — note it for the
human rather than papering over it here.
