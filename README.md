# devflow

A Claude Code plugin for taking a change from "here's what I want" to reviewed branches ready to
merge, through a graph of pieces small enough that you can actually review each one.

You approve four things and review every diff. Each approved node arrives as one commit on its own
branch; nothing reaches a shared branch unless you merge it yourself.

## How a run goes

| Phase | What happens | Ends with |
|---|---|---|
| **Intake** | Claude restates what you asked for, names its assumptions, and writes `goal.md` | your explicit yes |
| **Exploration** | It asks you for the context the goal needs — repos to change, plus anything to read — then reads it and writes `understanding.md` | no gate — knowledge, not a commitment |
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
holds the run's documents, and copies of the codebases and other material the run touches.

```sh
mkdir my-project && cd my-project
git init                       # the run's documents are yours to commit

printf '.devflow/\n' >> .gitignore                 # the run's working files stay out of git
```

That is all. **You do not clone anything up front.** Early in the run Claude asks what context
the goal needs and you decide — which repos the work will change, and anything else worth
reading: the service on the other side of an interface, a spec the change has to match, a design
doc. It fetches what you approved, records the list in `context.json`, and clones the repos that
actually get worked on later, once the plan is agreed.

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
├── context.json               # what you made available to read         (you decide)
├── understanding.md           # what it all actually looks like
├── design.md                  # how it will be done                     (approved)
├── graph.json                 # the work nodes                          (approved)
├── plans.md                   # the same graph, readable
└── .devflow/                  # never committed
    ├── state.json             # the run's phase and each node's status
    ├── context/<name>/        # what you gave it to read — never written to
    ├── repos/<name>/          # working clone; holds every node branch
    └── executors/<node>/      # one directory per node: its work item and its worktree
```

**`context.json` is committed; the material it names is not.** The list is small and it is the
record of what the run was based on, so it travels with the project. The repos and documents
themselves stay local.

**Don't delete `.devflow/` mid-run.** It is gitignored, but the working clone inside it holds
every finished node's branch — and until you merge or push them, that is the only copy of work
you already approved.

**Context is read-only and read-once.** Nothing in `.devflow/context/` is ever written to, and
no work node can target it. By the time execution starts, everything it held has been distilled
into `understanding.md` — so context is closed, and nothing goes back to re-read it.

Each node gets a worktree of the working clone on its own branch, `devflow/<node>`. The worktree
is removed once the node is done; **the branch stays** — that is the work.

A repo you are changing ends up in both `context/` and `repos/`, deliberately. The context copy
is whatever you pointed at — possibly your own working copy, mid-change — so devflow clones it
fresh to get a known starting commit. Your own checkout is never touched.

## Your part

**Every merge is yours.** Agents commit to their own node branch and nothing else — no merging,
no rebasing, no pushing, no pull requests, in any repository.

When a node is ready you get its diff and the Executor's note — what the tester ran, what the
reviewer said. You review it and tell Claude **approved**, **revise** with feedback, or
**reject**. There is nothing to commit: the work is already a single commit on the node's branch,
and approving it just marks the node done.

Before it shows you anything, it checks that the files are unchanged since the tester tested them
and the reviewer reviewed them, and sends the node back if they are not.

**You merge at two points.** When a node depends on several others whose branches have diverged,
there is no single commit to build on, so Claude asks you to merge those branches into your main
branch before it continues — however you like, including a pull request or a stack of them. And at
the end of the run, for whatever branches are left. Claude lists them in dependency order.

A chain of nodes needs no merging along the way: each one branches off the one before, so work
keeps moving while you review.

Nodes whose dependencies are done run in parallel, up to three at a time, so other work
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
