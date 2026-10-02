---
name: devflow-planning
description: The planning phase of a devflow run — decompose an approved design into a work graph, check it, and get the human's approval. Invoked by the devflow skill; not usually invoked directly.
---

# devflow — planning

Turning a design into an approved work graph. The layout, the state file and the standing
rules are in the `devflow` skill, which sends you here; they still apply.

You write the graph; the human approves it. Decompose, check your own work, show them what it
means, and only then start executing.

**The graph is frozen at approval.** Nothing changes it once execution begins — not a blocked
node, not a rejected one. Get it right here, because here is the only place it can be got
right. A run that turns out to need a different graph ends, and the next run plans afresh.

The same holds looking backwards: if decomposing shows the approved design to be wrong, stop
and say so rather than planning around it. The design is frozen too.

`graph.json` is yours to write. If one already exists from an earlier planning session, it is
a draft of your own: revise it rather than starting over.

### 1. Decompose the design into nodes

Read `design.md` if the project has one; otherwise work from what the human tells you. Read
`context.json` too — its `codebases` list is the only thing a node's `repo` may name.

**You consume context; you do not add to it.** If the design cannot be decomposed without
something nobody declared, that is exploration or the design having missed it. Say so and stop.
A plan that quietly widens its own inputs is the drift this whole design exists to prevent.

The exception is a **reference that has to become a working codebase** — the design turns out
to change the neighbouring service after all. That is not new context but a reclassification
of something already declared, and it has to happen before you ask for approval: invoke
`devflow-exploration` to re-declare it under `codebases`, then carry on.

Then split the work so that **every node is reviewable, verifiable, incremental and confined to
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

**The shape of the dependencies has its own cost.** Each node is built on a branch, and
where dependencies form a chain — `b` after `a`, `c` after `b` — each node branches straight
off the one before. Nothing has to be merged for work to keep moving, and the human is never
interrupted.

Where they fork and rejoin — `d` after both `b` and `c`, which ran in parallel — the two
branches have diverged and no commit contains both. Execution stops and asks the human to
merge them into the main branch before `d` can start.

So a join is a real interruption, and you are choosing between two costs:

- **A chain** gives up the parallelism between `b` and `c`. They run one after the other.
- **A fork that rejoins** keeps that parallelism and spends one human merge to get it.

Prefer the chain when the nodes are small enough that running them in sequence costs little —
the human's review time is the run's real bottleneck, not the machine's. Prefer the fork when
the two halves are substantial and genuinely independent. What you must not do is invent a
dependency that is not real just to avoid a join: a false dependency is wrong in the graph
forever, while a join costs one merge once.

### 2. Check it

Check your own graph before showing it to anyone, and fix what you find. Any of these is a
fault:

- **A cycle.** Work it out by hand: repeatedly set aside every node whose dependencies are
  all already set aside. If any nodes are left over, they are the cycle — name them.
- **A dependency on an id that does not exist.**
- **A duplicated id**, or an id that is not lowercase letters, digits and hyphens.
- **A `repo` that is not a `codebases` entry in `context.json`.** Check the manifest, not the
  disk — working clones do not exist yet; you create them at approval. A node pointing at a
  `references` entry is the error this catches, and it is fatal: reference material is read,
  never changed, so that node could never be built.
- **A node with no acceptance criteria**, or with none that can be checked by running
  something. "Auth works" is not a criterion; "an expired token yields 401" is.
- **A node with no test guidance.**

Three more are softer, but fix them too:

- **A title needing the word "and"** usually means two nodes.
- **A node with no `files_touched`** cannot be scheduled beside its siblings safely, so it
  will run alone.
- **Every node with two or more dependencies is a stop.** Count them — that is how many times
  execution will pause for the human to merge. If a chain would serve as well, use the chain.
  A node whose dependencies happen to form a chain already is not a stop, even though it lists
  several: one of those branches already contains the others.

### 3. Show them what it means

Write `plans.md` at the project root — the readable form of the same graph, generated from
it so the two cannot disagree:

```markdown
# Plan

<n> nodes across <m> codebases, with <k> merge checkpoints.

Waves below are what can run at the same time. Work flows from one wave to the next without
anything being merged — each node branches off the branch it depends on. The exceptions are
marked **checkpoint**: those depend on several branches that have diverged, so you will be
asked to merge before they start.

## Wave 1 — no dependencies

### <id> — <title>  (`<repo>`)
<intent>
- Accepts: <each criterion>
- Tests: <test guidance>

## Wave 2 — after <ids>

### <id> — <title>  (`<repo>`)  — **checkpoint: merge <branches> first**
...
```

Then tell the human what they are approving: how many nodes, how deep the chain is, which
nodes can run together, **how many times they will be asked to merge and at which nodes**, and
any judgement call you made while splitting the work that they might disagree with.

### 4. Get approval, then hand over

Only on their explicit approval — not "looks good", an actual yes:

1. **Provision a working clone for each codebase the graph names.** The set is the distinct
   `repo` values across all nodes — read it off the approved graph, do not decide it. For each,
   clone from the `source` that `context.json` declares:

   ```sh
   git clone <source> <project>/.devflow/repos/<name>
   ```

   **A fresh clone, even though the same codebase is already under `.devflow/context/`.** The
   context copy is whatever the human pointed at — possibly their own working copy, on a
   feature branch, with uncommitted changes — and a base commit from that means nothing.
   Execution needs a known branch at a known commit. Leave the context copy alone; it stays
   read-only.

   A declared codebase that no node targets is **not** cloned. It stays a reference.

   If a clone fails, stop and tell the human. Do not write `executing` with a codebase missing:
   every node targeting it would fail at dispatch.

2. Write `state.json`: `phase: "executing"`, and an entry for every node in the graph. A node
   whose dependencies are all `complete` starts `ready`; every other node starts `pending`.
3. Say the run is ready, and invoke the `devflow-execution` skill.

From here the statuses are execution's: it moves nodes through `running`, `complete`,
`blocked` and `abandoned` as work happens. You do not touch them again.
