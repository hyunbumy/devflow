# devflow — Harness

How the roles in [`design.md`](./design.md) are built on Claude Code.

`design.md` says what the Orchestrator and Executors do. This says what they *are*: which
agents exist, what each is allowed to do, how one starts and resumes another, and what is
enforced by Claude Code versus written down as instructions.

**The MVP adds no code of its own.** Everything is built from Claude Code's own constructs —
agent definitions, a skill, and templates — plus `git`.

---

## 1. The agents

| Agent | Runs as | Tools | Job |
|---|---|---|---|
| Orchestrator | The human's interactive session, in the project directory, following the `devflow` skill | All | The run: goal, design, graph, scheduling, state, confirm relay, worktrees |
| Executor | A subagent, one per node | Read, Edit, Write, Bash, Grep, Glob, Agent | Implement the node; start a tester and a reviewer for those stages |
| Tester | A subagent of the Executor, new for every test stage | Read, Bash, Grep, Glob | Run the tests, report what ran and what happened |
| Reviewer | A subagent of the Executor, new for every review stage | Read, Grep, Glob | Review the diff, report findings |

**Tool lists are enforced by Claude Code.** An agent cannot call a tool its definition does not
list. That is what makes the reviewer genuinely unable to change code: it has no tool that
writes. The tester is weaker — it needs a shell to run tests, and a shell can write, so the
tester is kept from editing by instruction.

A tool listed in a definition but not present in the harness is silently dropped, so a
definition should not assume every listed tool arrives.

### Why the Executor implements, and starts the others fresh

A subagent the Executor **starts** keeps the Executor waiting until it reports, and its report
comes back to the Executor. A subagent the Executor only **messages** — resuming one it started
earlier — does not hold the Executor open, and its reply can arrive at the Orchestrator instead.

So the design avoids messaging downward entirely:

- **The Executor is the implementer.** It keeps its own context across every loop, and nothing
  implementation-related is ever handed off and re-read.
- **Testers and reviewers are always started, never resumed.** Their reports reliably return to
  the Executor. Neither holds anything worth keeping between rounds: a tester runs a command,
  and a reviewer with fresh eyes reviews the whole diff instead of checking its old list.
- **The only agent that is messaged is the Executor**, by the Orchestrator, for revision rounds —
  where the reply is meant to reach the Orchestrator.

---

## 2. The Executor's scratch directory

Each node gets a directory holding its work item and its worktree:

```
.devflow/executors/<node-id>/
├── work_item.md          # the node contract, including the base commit (Orchestrator-owned)
└── <repo>/               # worktree of .devflow/repos/<repo> on branch devflow/<node-id>
```

A subagent's working directory is the Orchestrator's, so the scratch directory is a location the
Executor is *told about* — by absolute path, in its work item — not one it is started in.
Keeping `work_item.md` beside the worktree keeps it out of the diff under review.

The worktree's `.git` is a file pointing at `.devflow/repos/<repo>/.git`, so every git command
the Executor runs reaches the shared repository. That is what lets it commit without anything
being pushed or fetched, and it is also why it is told to touch no branch but its own (§4).

---

## 3. One node, end to end

**Dispatch** — the Orchestrator, following the skill:

1. Write the node's state: `running`, Executor `working` (invariant I5 — before anything else).
2. Work out the **base commit** (design.md §5.3): the main branch for a node with no
   dependencies, the dependency's branch tip for one, the merged main branch for several. A node
   needing several waits for the human's checkpoint merge, verified first (design.md §8.3).
3. Add the worktree on a new branch:
   `git -C .devflow/repos/<repo> worktree add -b devflow/<id> <scratch>/<repo> <base-commit>`.
4. Write `work_item.md` from its template: the node's intent, acceptance criteria and test
   command from `graph.json`, the absolute scratch path, the branch, the base commit, and the
   rules.
5. Start an Executor with the work item as its prompt.

A crash between steps leaves a node marked `running` with no Executor — conservatively stale,
which recovery handles (§5).

**The loop** — the Executor:

1. **Implementation.** Make the change and its tests, then **commit** it to the branch.
2. **Test.** Start a new tester with the worktree path, the commit under test, the node's
   `tests` guidance, and the commands the previous tester ran, if any. It chooses what to run —
   at least what was run before — and returns pass or fail, the exact commands, their output,
   the commit it tested, and whether the run left the tree dirty. Fail → back to 1. Third
   failure → `blocked`.
3. **Review.** Start a new reviewer with the worktree path, the diff, the node's acceptance
   criteria, and any earlier findings. It returns findings and the commit it reviewed. Findings
   → back to 1, which means committing, testing and reviewing again. Second round with findings
   → `blocked`.
4. **Confirm.** Squash the branch to one commit — `git reset --soft <base> && git commit` — and
   finish, with a completion note: outcome, the commands the tester ran and their result, the
   reviewer's result, the commit both checked, and anything the human needs.

**Completion** — the Orchestrator, when the Executor's final message arrives: record Executor
`done`, then present the diff and the completion note to the human.

Before presenting, the Orchestrator runs `git diff --quiet <commit> HEAD` for the commit the
tester named and again for the commit the reviewer named. A difference means something was edited
after it was checked, and the node goes back. The comparison is of content rather than commit
identity because the Executor squashes before reporting — a squash changes the commit and no
files, so it passes, while an edit does not. This is what an earlier draft tried to get from
hashing the working tree, which misfired on generated files (design.md §12, item 11); comparing
two commits cannot. The Orchestrator still reads the completion note and sends a node back when
the tests it names do not match what the node needed.

**Revision.** The Orchestrator records Executor `working` and sends the human's feedback to the
same Executor as a message. It resumes with its context intact and goes round the loop again,
starting new testers and reviewers as before.

**Approval.** There is nothing to land — the commit is already on the branch. The
Orchestrator marks the node `complete`, removes the worktree with
`git -C .devflow/repos/<repo> worktree remove <scratch>/<repo>` (never `rm -rf`, which leaves a
stale registration needing `git worktree prune`), keeps the branch, and works out which nodes are
now ready. On rejection it deletes the branch too.

**Merging** is the human's, at the checkpoints design.md §8.3 describes. The Orchestrator asks,
waits, and verifies with `git merge-base --is-ancestor` before dispatching the node that was
waiting.

**Which Executor is working which node** is kept in the Orchestrator's own session, not in
`state.json`. A subagent does not outlive the session that started it, so a stored handle would
be useless after a restart.

---

## 4. Confinement

**What the MVP relies on.** The Executor's definition and its work item both say: every file
created or changed must be inside the scratch directory; never touch another node's worktree, the
codebase checkout, `.devflow/context/`, or the project root. Commit only to your own branch — no
merge, no rebase onto anything else, no push, no branch or ref operation on anything but
`devflow/<your id>`.

**An Executor is given no context beyond its work item** — not `.devflow/context/`, not
`understanding.md`, not the design. Everything it needs has to be in the work item, which is why
design.md §5.1 makes `intent` carry any constraint that is not obvious from the code, and why a
node needing more than that is a badly planned node rather than a reason to widen what the
Executor reads. Reference material is for the phases that decide *what* to build; by execution it
has done its job (I9).

**What that does not guarantee.** A subagent works in the Orchestrator's directory with the
Orchestrator's reach. Nothing but instructions stops an Executor that is confused about where
it is from writing elsewhere.

**Sharing a repository adds a second way out.** A worktree's `.git` points at the codebase's
repository, so `git` run from inside the worktree reaches every node's branch. An Executor can
delete or move a branch it does not own without ever writing outside its own directory — a path
check would not see it. Clones made this impossible; that is what the change traded away
(design.md §8.1).

**The consequence, accepted.** An Executor that escapes can damage another node's worktree or
branch, the codebase checkout (invariant I8), or the project itself. That would be serious
when it happens. For an MVP it is a risk taken knowingly and addressed if it occurs; §7 lists
what to reach for.

---

## 5. Permissions and recovery

**Permissions.** All agents run inside the human's interactive session, so a permission prompt
raised by any of them reaches the human rather than being silently refused. No per-agent
permission configuration is needed.

**Recovery.** Subagents do not survive the session. When a restarted Orchestrator finds a node
`running`:

- **Executor `working`:** start a fresh Executor with the same work item. It reads `git log` on
  its branch against the base commit to see which stages the previous one committed, and
  continues. The previous Executor's reasoning is lost; its committed output is not.
- **Executor `done`:** the completion note died with the session, so nothing records what was
  tested. Start a fresh Executor told the implementation is finished; it goes straight to test
  and review, then confirms. The branch tip tells it what to test. If the human had already
  approved before the crash, mark the node `complete`.

The worktree registration survives a restart too, so `git -C .devflow/repos/<repo> worktree list`
is a durable record of which nodes have live trees — worth checking it against `state.json`, and
pruning any worktree whose node is `complete`.

---

## 6. What is enforced, and what is written down

| What | Guaranteed by |
|---|---|
| The reviewer cannot change code | Its tool list — **enforced** |
| Tests are run by an agent other than the implementer | The loop structure — the Executor must start a tester to get a result |
| What the human sees is what was tested and reviewed | The tester and reviewer each name the commit they checked; the Orchestrator runs `git diff --quiet` against the branch tip for each — **checkable**, survives the squash, and immune to generated files (design.md §7.2) |
| The tester changes nothing | Its instructions; it reports a dirty tree, which is what catches a run that rewrote a tracked file without moving the commit |
| The tester's choice of tests is sound | Its instructions; its report lists the exact commands, and a retest must cover at least the previous round's |
| The Executor does not skip a stage or invent a report | Its instructions; the completion note must carry both reports' results, which makes a skip visible |
| Budgets: tests 3, review 2 | The Executor's instructions |
| The Executor commits only to its own branch | Its instructions. A worktree reaches every branch in the shared repository, and no tool list or path check narrows that (§4) |
| Graph validation, base commits, ready nodes, dispatch, checkpoint verification, `state.json` writes | The Orchestrator's skill, as explicit numbered steps |

Everything below the first two rows is performed by an agent following written steps. That holds
up for a handful of nodes and plainly worded procedures. Where it breaks in practice, the fix is
to move that one operation into code (design.md §12, item 13).

### The artifacts

devflow ships as a **Claude Code plugin** in its own repository. A project loads the plugin
rather than copying files into its `.claude/`, so every project runs the same version.

| File | Contents |
|---|---|
| `agents/executor.md` | The loop above, the rules, the tool list |
| `agents/tester.md` | How to choose what to run, and its report format |
| `agents/reviewer.md` | The review checklist — written out in full, since `implementation.md` does not exist inside a project — and its report format |
| `skills/devflow/SKILL.md` | The Orchestrator's phases and gates, and the layout and standing rules that hold in all of them |
| `skills/devflow-exploration/SKILL.md` | The context step — proposing what the goal needs, writing `context.json`, fetching what the human approved — and then reading it |
| `skills/devflow-planning/SKILL.md` | Decomposition, graph validation against `context.json`, and provisioning a working clone per codebase the graph names |
| `skills/devflow-execution/SKILL.md` | Base commits, dispatch, the confirm relay, completing a node, checkpoint merges, and recovery |
| Templates | Work item, tester report, reviewer report, completion note |

---

## 7. Stronger isolation, for later

From least to most isolation. None is in the MVP.

### A. A hook on file-writing tools

A pre-tool hook checks every write's path against the calling Executor's scratch directory. Hook
payloads identify the calling agent, so the Orchestrator can record which agent owns which
directory and the hook can look it up.

- **Catches:** writes through file-writing tools, with a clear refusal the agent can act on.
- **Misses:** anything written through the shell. No match on command text closes that — `cd`,
  variables, `tee`, `cp`, heredocs and subshells all reach the same path.
- **Cost:** a short script and an agent-to-directory mapping. Cheap, but partial.

### B. Executors as separate processes

Start each Executor as its own `claude -p` process, with its working directory set to the scratch
directory. Claude Code then confines the session to that directory.

- **Catches:** file tools, and shell commands it can analyse — copies, directory creation,
  redirects, pipes — for reads as well as writes. A neighbouring directory in the same project is
  refused exactly like `/tmp`.
- **Misses:** a process started by a command the Executor is allowed to run. A test script can
  still write anywhere, and an Executor must run tests. It also misses branch damage entirely: the
  worktree is inside the scratch directory, so `git` run there is a legitimate write to a
  permitted path no matter which ref it moves (§4).
- **Cost:** process lifecycle becomes devflow's job. Completion is no longer delivered to the
  Orchestrator automatically. A session id per node must be stored, which does let recovery resume
  rather than restart. Permission prompts go unanswered, so any command not allow-listed up front
  becomes an escalation.

### C. B, inside an operating-system sandbox

Run the process under `bwrap` — or `podman`, `systemd-run`, `unshare` — with only the scratch
directory writable.

- **Catches:** everything B misses *except* branch damage, and the worktree makes it harder than
  it was with a clone: the shared repository at `.devflow/repos/<repo>/.git` has to be writable
  for the Executor to commit at all, so the sandbox cannot simply exclude it. Splitting the
  difference means allowing writes to that repository but not to other nodes' refs, which git
  offers no way to express by path.
- **Cost:** deciding what each toolchain legitimately needs outside the scratch directory —
  `~/.cargo`, `~/.npm`, `/tmp` — and debugging builds that fail inside the sandbox but work
  outside it. The work is the list of bindings, not the wrapper.

### D. Commits brokered instead of run

Withhold `git` from the Executor and give it one operation — "commit what is in my worktree" —
through a tool that hard-codes the branch. This is the only option that closes branch damage
rather than narrowing it, because the Executor stops being able to name a ref at all.

- **Catches:** every ref operation, including ones that never leave the scratch directory.
- **Cost:** the Executor needs `git diff` and `git log` to do its job, so the broker has to serve
  reads as well; and it is real code, which the MVP does not have (design.md §12, item 13).

### Considered and dropped

**Checking afterwards that only the node's own files changed.** Hashing everything outside the
scratch directories before and after a run detects a change but cannot attribute it to a
particular Executor, so it reports suspicion rather than fact.
