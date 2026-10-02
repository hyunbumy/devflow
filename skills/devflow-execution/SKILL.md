---
name: devflow-execution
description: The execution phase of a devflow run — dispatch work nodes to Executors in their own worktrees and branches, relay their diffs for human confirmation, and ask the human to merge at the checkpoints the graph requires. Invoked by the devflow skill; not usually invoked directly.
---

# devflow — execution

Driving an approved graph to reviewed, committed branches. The layout, the state file and the
standing rules are in the `devflow` skill, which sends you here; they still apply.

## On start, or when asked to resume

**You own the statuses while the run is executing** — planning sets them when it hands over,
and you move them from there. **Never edit `graph.json`.** It was frozen when the human
approved it; a run that needs a different graph is a run that has to end.

1. Read `graph.json` and `.devflow/state.json`. If `state.json` will not parse, stop and ask
   the human — do not guess it back into shape.
2. **See what is actually on disk before you restart anything.** For each distinct `repo` the
   graph names — those are the only ones that were cloned (planning provisions from the graph):

   ```sh
   git -C <project>/.devflow/repos/<repo> worktree prune   # drop registrations for gone directories
   git -C <project>/.devflow/repos/<repo> worktree list
   git -C <project>/.devflow/repos/<repo> branch --list 'devflow/*'
   ```

   Now you know which nodes have a live worktree and which have a branch. A `running` node with no
   worktree cannot have an Executor started into it — say so rather than guessing, and check
   whether its branch still holds the work before deciding anything.

   **If `.devflow/repos/<repo>` is gone altogether**, every node branch for that codebase went
   with it, because they lived in that repository. How bad that is depends on what had finished:

   - **No node for it is `complete`** — nothing is lost. Re-clone it from the `source` in
     `context.json`, exactly as planning would have, and carry on. Any `running` node for it
     starts over from its work item.
   - **Some node for it is `complete`** — that approved work is gone unless the human pushed the
     branch somewhere. **Stop and tell them**, naming which nodes are affected. Do not re-clone:
     a fresh clone would look healthy while silently missing landed work, and every dependent
     would then be built on a base that lacks it. Recovering a pushed branch is theirs to do; if
     it was never pushed, the work is gone and the run cannot honestly continue past it.

3. Act on what each entry says:

   | Recorded | Do |
   |---|---|
   | `pending`, `ready` | Nothing was in flight; schedule normally |
   | `running`, executor `working` | That Executor is gone. If its worktree is still there, start a fresh one with the same work item, telling it the implementation may be partly done and to read `git log <base>..HEAD` on its branch first |
   | `running`, executor `done` | Its report died with the session. Start a fresh Executor, telling it the implementation is finished and to go straight to test and review |
   | `complete` | Remove the worktree if it is still there (see "Completing a node"). Keep the branch |
   | `blocked`, `abandoned` | Leave it; mention it to the human. Nothing in this run revives it |

4. Tell the human where the run stands, including which branches are finished and unmerged, then
   continue.

## What to dispatch

A node is dispatchable when **every node it depends on is `complete`** and you have a base commit
for it (see below). Among those, do not start a node if:

- **three Executors are already running**, or
- **another running node touches the same codebase and its `files_touched` overlaps.** Two
  nodes in different codebases never overlap.

## The base commit

Every node branches from one commit, and that commit must already contain all of its
dependencies' work. Work it out before anything else, because it decides whether the node can
start at all.

**`main`** below means whatever branch `.devflow/repos/<repo>/` has checked out. Never switch it.

| Dependencies | Base commit |
|---|---|
| none | the tip of `main` |
| one | the tip of `devflow/<that dependency>` |
| several | see below — usually `main`, after a merge |

**With several dependencies**, first look for one branch that already contains all the others —
which is what a chain of nodes produces, as opposed to a fork. Take each dependency in turn as the
candidate and test every other against it:

```sh
git -C <project>/.devflow/repos/<repo> merge-base --is-ancestor devflow/<other> devflow/<candidate>
```

Exit 0 means the candidate contains that other. A candidate that contains every other is the base:
use its tip and there is nothing more to do.

Otherwise the branches have diverged and no commit contains all of them. **Stop and ask the
human for a checkpoint merge.** Tell them:

- which node is waiting, and which branches have to come together;
- that you need them merged into `main`, by whatever git flow they prefer — a local merge, a pull
  request, a stack of pull requests;
- that you will carry on with other nodes meanwhile, and will check when they say it is done.

Leave the node `ready`. It holds no concurrency slot while it waits. Do not merge anything
yourself, and never dispatch a node onto a base that is missing a dependency.

**When they say they have merged**, verify before dispatching:

1. Pull, so that a merge made through a remote becomes visible locally:

   ```sh
   git -C <project>/.devflow/repos/<repo> pull --ff-only
   ```

   The clone was made from the source `context.json` declares, so it always has an upstream. You
   do not know which way they merged and you do not need to: if they used a pull request this
   brings it in, and if they merged locally in this clone the local branch is already ahead and
   the pull reports "already up to date". Either way step 2 is what proves it.

   If it will not fast-forward, stop and tell the human — never reset or merge it yourself.

2. Check each dependency is reachable from `main`:

   ```sh
   git -C <project>/.devflow/repos/<repo> merge-base --is-ancestor devflow/<dep> main
   ```

   **A failure here is not proof the merge is missing.** A squash merge creates a commit that no
   branch tip is an ancestor of, so this check fails even though the work is there. Say which
   dependency you could not find and ask. If they say they squashed, take their word and carry
   on. What you must never do is dispatch the node while genuinely unsure.

## Dispatch

In this order. Never start an Executor before step 1 is on disk.

1. **Write state**: that node `running`, executor `working`.
2. **Resolve the base commit to a hash**, so it cannot shift under the node:

   ```sh
   git -C <project>/.devflow/repos/<repo> rev-parse <base ref>
   ```

3. **Add the worktree on a new branch:**

   ```sh
   git -C <project>/.devflow/repos/<repo> worktree add -b devflow/<id> \
     <project>/.devflow/executors/<id>/<repo> <base-commit>
   ```

   **If `devflow/<id>` already exists**, this node was dispatched before and its branch holds the
   earlier Executor's commits. Check the branch out instead of creating it, and do not pass a base
   commit — that would throw the work away:

   ```sh
   git -C <project>/.devflow/repos/<repo> worktree add \
     <project>/.devflow/executors/<id>/<repo> devflow/<id>
   ```

   Take the base commit for the work item from the existing `work_item.md`, not from the graph
   again; the branch starts where it started.

4. **Write `work_item.md`** into the node's directory, using the template below.
5. **Start a `devflow:executor` subagent**, passing the contents of `work_item.md` as its
   prompt plus a line naming the node id.

Then carry on: dispatch anything else that qualifies, or talk to the human.

## When an Executor reports

**`blocked`** → write `blocked`, and mark every node that depends on it `blocked` too. Tell
the human what stopped it and which nodes are now stalled. Do not retry it yourself.

**`confirm`** → **check it before you show the human anything.**

The completion note names the commit the tester tested and the commit the reviewer reviewed. For
each, confirm the files are unchanged since:

```sh
git -C <worktree> diff --quiet <that commit> HEAD
```

Compare content, not commit hashes. The Executor squashes before reporting, so the commit the
reviewer saw no longer exists — but a squash rewrites history without touching a file, so this
comparison still passes. A difference means something was edited after being checked: send the
node back for another round of test and review rather than passing it on.

Also read the note itself. If the tests it names do not match what the node needed, or either
commit is missing, send it back. Do not pass a gap to the human.

Then present:

1. The diff, from `git -C <worktree> diff <base-commit> HEAD`. The work is committed, so that is
   the whole change — there are no untracked files to chase.
2. The completion note, as the Executor wrote it.
3. What you need from them: review it, then tell you **approved**, **revise** with feedback, or
   **reject**. They do not need to commit or land anything.

Write executor `done` before you present.

## What the human comes back with

**revise** → write executor `working`, then send their feedback to that same Executor as a
message. It keeps its context and goes round its loop again.

**reject** → write `abandoned`, remove the worktree, and **delete the branch**
(`git -C <project>/.devflow/repos/<repo> branch -D devflow/<id>`) — a rejected node leaves nothing
behind. Mark every node that depends on it `blocked`. Tell the human this node and its dependents
are finished for this run, and that the work needs a fresh run to plan differently. Do not touch
the graph.

**approved** → run the steps under "Completing a node".

## Completing a node

There is nothing to land. The Executor already committed the work to `devflow/<id>`, and approval
means the human accepted that commit. So:

1. **Write `complete`.**
2. **Remove the worktree**, keeping `work_item.md` and keeping the branch:

   ```sh
   git -C <project>/.devflow/repos/<repo> worktree remove <project>/.devflow/executors/<id>/<repo>
   ```

   Use this, not `rm -rf` — deleting the directory leaves a registration behind that then needs
   `git worktree prune`. If the worktree has changes git refuses to discard, it means something is
   there that was not in the approved commit: look before reaching for `--force`.

   **Never delete the branch.** It is the node's output, and its dependents branch from it.

3. **Work out what became dispatchable** and carry on. A node whose last dependency just completed
   may now have a base commit, or may need a checkpoint merge — go back to "The base commit".

## Ending the run

When no node can be dispatched and none is running, the run is over. Tell the human what is left
to merge:

- every `complete` node's branch not yet reachable from `main` — test each with
  `git -C <project>/.devflow/repos/<repo> merge-base --is-ancestor devflow/<id> main`, where a
  non-zero exit means it still needs merging — **listed in dependency order** so they know what
  merges into what;
- which nodes are `blocked` or `abandoned`, and what that leaves unbuilt.

Merging is theirs. Do not merge, push, or open a pull request, and do not delete a branch that has
not been merged.

## The work item

```markdown
# Work item — <node id>

**Scratch directory:** <absolute path of .devflow/executors/<id>/>
**Worktree:** <absolute path of the worktree>
**Branch:** devflow/<node id>
**Base commit:** <the commit the branch starts at>
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
