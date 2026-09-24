# devflow

A Claude Code plugin for taking a change from "here's what I want" to landed code, through a
graph of pieces small enough that you can actually review each one.

You approve four things and review every diff. Nothing is committed on your behalf, ever.

## How a run goes

| Phase | What happens | Ends with |
|---|---|---|
| **Intake** | Claude restates what you asked for, names its assumptions, and writes `goal.md` | your explicit yes |
| **Exploration** | It reads the codebases the goal touches and writes `understanding.md` | no gate — knowledge, not a commitment |
| **Design** | It proposes how, with the alternatives it rejected, in `design.md` | your explicit yes |
| **Planning** | It splits the design into work nodes: `graph.json`, plus `plans.md` to read | your explicit yes |
| **Execution** | One Executor per node: implement, test, review, then hand you the diff | your review of each node |

A run only walks forward. Once you approve something it is frozen; if a later phase shows it
was wrong, the run ends and the next one starts from what this one learned.

## Inside one node

The Executor writes the code. It does not test its own work or review it: for each of those
it starts a fresh agent.

- The **tester** runs the tests and reports the exact commands it ran. It has no editing tools.
- The **reviewer** reads the diff with fresh eyes and reports findings. It has no tools that
  change files at all.

Any failure — failing tests, review findings, your revision feedback — sends the node back to
implementation, and the whole loop runs again. Budgets: three test failures or two rounds of
review findings and the node stops as blocked.

## Setting up a project

A devflow project is a directory of its own. It is not the codebase you are changing — it
holds the run's documents, and copies of the codebases the run touches.

```sh
mkdir my-project && cd my-project
git init                       # the run's documents are yours to commit

mkdir -p .devflow/repos
git clone <your codebase> .devflow/repos/<name>    # one per codebase the work touches

printf '.devflow/\n' >> .gitignore                 # the run's working files stay out of git
```

Then open Claude Code in that directory **once, interactively**, and accept the trust prompt —
a workspace that has never been trusted refuses the writes a run needs.

Load the plugin, either with `--plugin-dir /path/to/devflow` or by installing it, and start:

```
/devflow  I want the api service to stop accepting expired tokens.
```

A bare `/devflow` in an existing project picks up wherever the run left off.

## What ends up where

```
my-project/                    # you commit everything here
├── goal.md                    # what the run is for                     (approved)
├── understanding.md           # what the codebases actually look like
├── design.md                  # how it will be done                     (approved)
├── graph.json                 # the work nodes                          (approved)
├── plans.md                   # the same graph, readable
└── .devflow/                  # never committed
    ├── state.json             # the run's phase and each node's status
    ├── repos/<name>/          # your codebase copies — read-only to the run
    └── executors/<node>/      # one directory per node: its work item and its clone
```

## Your part

**Every commit is yours.** No agent commits, merges, rebases or pushes, in any repository.

When a node is ready you get its diff and the Executor's note — what the tester ran, what the
reviewer said. You review it, commit it in the node's clone, land it into your codebase copy,
and tell Claude **approved**, **revise** with feedback, or **reject**. On approved it checks
the change really landed before marking the node done.

Nodes whose dependencies have landed run in parallel, up to three at a time, so other work
continues while you review.

## What this does not do

- **Change the plan mid-run.** A node that cannot be built stops the node, not the run; the
  rest continues and the next run plans differently.
- **Enforce that an Executor stays in its own directory.** It is told to, and it does; nothing
  stops it. See `docs/harness.md` §7 for the stronger options and what they cost.
- **Prove the diff you review is the one that was tested.** An earlier version hashed it;
  generated files made that misfire. See `docs/design.md` §12, item 11.

## Reading further

- [`docs/design.md`](docs/design.md) — the architecture: roles, invariants, the run lifecycle,
  and every open question with the reasoning behind it.
- [`docs/harness.md`](docs/harness.md) — how the roles map onto Claude Code, what is enforced
  by the tool and what rests on instructions.
