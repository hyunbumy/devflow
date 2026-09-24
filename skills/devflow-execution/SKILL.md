---
name: devflow-execution
description: The execution phase of a devflow run — dispatch work nodes to Executors, relay their diffs for human confirmation, and land approved work. Invoked by the devflow skill; not usually invoked directly.
---

# devflow — execution

Driving an approved graph to landed code. The layout, the state file and the standing rules
are in the `devflow` skill, which sends you here; they still apply.

## On start, or when asked to resume

**You own the statuses while the run is executing** — planning sets them when it hands over,
and you move them from there. **Never edit `graph.json`.** It was frozen when the human
approved it; a run that needs a different graph is a run that has to end.

1. Read `graph.json` and `.devflow/state.json`. If `state.json` will not parse, stop and ask
   the human — do not guess it back into shape.
2. Act on what each entry says:

   | Recorded | Do |
   |---|---|
   | `pending`, `ready` | Nothing was in flight; schedule normally |
   | `running`, executor `working` | That Executor is gone. Start a fresh one with the same work item, telling it the implementation may be partly done and to read the clone first |
   | `running`, executor `done` | Its report died with the session. Start a fresh Executor, telling it the implementation is finished and to go straight to test and review |
   | `complete` | Delete the clone if it is still there |
   | `blocked`, `abandoned` | Leave it; mention it to the human. Nothing in this run revives it |

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

**`confirm`** → present it to the human:

1. The diff, from `git -C <clone> diff <base-commit>` plus any new untracked files.
2. The completion note, as the Executor wrote it — the commands the tester ran, their result,
   and the reviewer's verdict.
3. What you need from them: review it, land it, then tell you **approved**, **revise** with
   feedback, or **reject**.

Write executor `done` before you present.

Nothing verifies mechanically that the diff you show is the diff that was tested and
reviewed; that rests on the Executor following its loop, and on the tester reporting the
commands it actually ran. Read the completion note before relaying it. If the tests it names
do not match what the node needed, or the reviewer's verdict is missing, send it back rather
than passing the gap to the human.

## What the human comes back with

**revise** → write executor `working`, then send their feedback to that same Executor as a
message. It keeps its context and goes round its loop again.

**reject** → write `abandoned`, and mark every node that depends on it `blocked`. Tell the
human this node and its dependents are finished for this run, and that the work needs a fresh
run to plan differently. Do not touch the graph.

**approved** → they have already committed and landed the change. Run the landing steps.

## Landing

1. **Refresh your copy**, but only if it tracks something. Check first:

   ```sh
   git -C <project>/.devflow/repos/<repo> rev-parse --abbrev-ref --symbolic-full-name @{u}
   ```

   - **It names an upstream** → `git -C <project>/.devflow/repos/<repo> pull --ff-only`. If
     that will not fast-forward, stop and tell the human. Never reset or merge it.
   - **It fails** → the copy has no upstream, so the human landed into it directly. Nothing to
     pull. Carry on to step 2, which is what actually proves the change arrived.

2. **Check the change actually landed.** Write the node's committed diff to a patch and test
   whether it can be reversed out of the copy:

   ```sh
   git -C <clone> diff <base-commit> HEAD > <project>/.devflow/executors/<id>/landed.patch
   git -C <project>/.devflow/repos/<repo> apply --reverse --check \
     <project>/.devflow/executors/<id>/landed.patch
   ```

   Success means the change is present. **This step is the proof, not step 1** — never mark a
   node complete without it.

   Failure means the copy does not contain what you are about to call landed. Say so and ask
   the human what happened. If they say they squashed it or edited while landing, take their
   word; if they expected it to be there, it is not, and something went wrong.

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
