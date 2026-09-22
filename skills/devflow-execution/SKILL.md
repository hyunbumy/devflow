---
name: devflow-execution
description: The execution phase of a devflow run — dispatch work nodes to Executors, relay their diffs for human confirmation, and land approved work. Invoked by the devflow skill; not usually invoked directly.
---

# devflow — execution

Driving an approved graph to landed code. The layout, the state file and the standing rules
are in the `devflow` skill, which sends you here; they still apply.

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
