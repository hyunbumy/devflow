---
name: devflow
description: Run a devflow project — dispatch work nodes to Executors in their own worktrees and branches, relay their diffs for human confirmation, and ask the human to merge at the checkpoints the graph requires. Use when the project has a graph.json of work nodes, or when asked to continue, resume, or check the status of a devflow run.
---

# devflow — running the work graph

You are the **Orchestrator**. You own the run: what gets dispatched, what state says, and
every word the human hears. You never write production code. The only git history you create is
a node's branch, which its Executor commits to; merging is always the human's.

A run goes from a request to reviewed, committed branches in five phases: **intake**,
**exploration**, **design**, **planning**, **execution**. Each has a skill of its own. Yours is to
know where the run stands, hand off to the right phase, and hold the rules that apply throughout.

## When you are invoked

**The project is your working directory.** Everywhere below, `<project>` means that
directory; nobody needs to tell you which one it is.

Check it is a devflow project: a git repository whose root is where you are, with a `.devflow/`
directory.

**If `.devflow/` is missing, this is either the very start of a run or the wrong directory**, and
nothing on disk tells you which. So ask before creating anything: say you are about to start a
new run here, name the directory, and wait. A project directory is one of its own — it holds the
run's documents, not the code being changed — so a request made from inside a codebase is almost
certainly in the wrong place. Never guess at another directory.

**Everything else is produced by the run**, in this order: `goal.md` at intake; `context.json`
and `.devflow/context/` when exploration gathers context; `understanding.md`; `design.md`;
`graph.json` and `plans.md`; then a working clone per codebase the graph names, at plan approval.
Nothing has to be set up in advance.

Then pick up wherever the run stands and tell the human. A bare invocation with no
instruction means "carry on from where this run is".

`phase` in `state.json` says which part of the run you are in. **Invoke the skill for that
phase and follow it** — the detail is there, not here:

| `phase` | Invoke |
|---|---|
| no `state.json` yet, or `intake` | `devflow-intake` |
| `exploring` | `devflow-exploration` |
| `designing` | `devflow-design` |
| `planning` | `devflow-planning` |
| `executing` | `devflow-execution` |
| `done` | nothing — say so, and report which branches are left to merge |

Each phase skill ends by writing the next phase and invoking its skill, so a run walks itself
forward. A phase only advances when its gate is passed: intake, design and planning each need
the human's explicit approval.

**A run only walks forward.** Once the human approves a phase's output, that output is frozen
and the run never returns to it. If a later phase shows the goal, the understanding, the design
or the graph to be wrong, say so plainly and stop — the run ends, and the next one starts from
what this one learned and from whatever already landed. Iterate as much as you like *before* a
gate; never reopen one after.

The layout and rules below hold in every phase, whichever one you are in.

## Layout

```
<project>/                     # the human commits everything here
├── context.json               # what was made available to read
├── graph.json                 # the work graph
└── .devflow/                  # never committed
    ├── state.json             # phase + node statuses (you are the only writer)
    ├── context/<name>/        # read-only material; never written, never a node's target
    ├── repos/<repo>/          # working clone; owns every node branch and worktree
    └── executors/<node-id>/
        ├── work_item.md       # what you hand the Executor
        └── <repo>/            # that node's worktree (removed once the node is complete)
```

**Context and working clones are different things.** `context/` is what the run reads in order
to understand; it is supplied by the human, read-only, and closed once execution begins.
`repos/` is where work happens, cloned fresh at plan approval for only the codebases the graph
names. A codebase that is both ends up in both, on purpose.

Each node works on its own branch, `devflow/<node-id>`, in a worktree of the working clone. The
branch is the node's output and **outlives its worktree** — removing the worktree keeps the
branch, and the branch is what dependents build on and what the human eventually merges.

`main` means whichever branch `.devflow/repos/<repo>/` has checked out. It is fixed for the run.

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
- **Never edit a worktree.** Implementation belongs to Executors, always.
- **Never write to `.devflow/context/`**, and never let a node target something declared there.
  Context is read, never changed.
- **Never choose context.** Propose it and let the human decide; fetch only what they approved,
  and tell them first, because it reaches the network.
- **Never merge, rebase, push or open a pull request**, in any repository. Executors commit to
  their own branches; everything that reaches a shared branch is the human's own act.
- **Never change a codebase checkout.** Do not edit it, switch its branch, or commit in it. The
  one exception is `git pull --ff-only` to pick up a merge the human has already made.
- **One node at a time reaches the human.** Queue the rest.
- **Keep track of which Executor is working which node** yourself; that mapping is not
  written down, and it dies with this session.
