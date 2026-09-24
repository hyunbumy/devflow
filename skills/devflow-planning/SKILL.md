---
name: devflow-planning
description: The planning phase of a devflow run — decompose an approved design into a work graph, check it, and get the human's approval. Invoked by the devflow skill; not usually invoked directly.
---

# devflow — planning

Turning a design into an approved work graph. The layout, the state file and the standing
rules are in the `devflow` skill, which sends you here; they still apply.

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

**`files_touched` must include the test files the node will change.** New behaviour needs a
new test, and that test lives somewhere — if the file holding it is not declared, the Executor
must either skip the test or work outside its declared scope. Both are bad, and the second is
worse: `files_touched` is what decides which nodes may run at the same time, so an Executor
writing outside it can collide with a sibling.

**Check what the existing tests already assert** before writing acceptance criteria. A
criterion that contradicts a test currently in the tree gives the Executor an impossible node:
it cannot satisfy the criterion and leave the suite untouched. If a node must change an
existing expectation, say so in `intent` and name that file in `files_touched`.

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
