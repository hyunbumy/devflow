# devflow — Harness

How the roles in [`design.md`](./design.md) are built on Claude Code.

`design.md` says what the Orchestrator and Executors do. This says what they *are*: which
agents exist, what each is allowed to do, how one starts and resumes another, and what is
enforced by Claude Code versus written down as instructions.

**The MVP adds no code of its own.** Everything is built from Claude Code's own constructs —
agent definitions, a skill, and templates — plus `git` and `sha256sum`.

---

## 1. The agents

| Agent | Runs as | Tools | Job |
|---|---|---|---|
| Orchestrator | The human's interactive session, in the project directory, following the `devflow` skill | All | The run: goal, design, graph, scheduling, state, confirm relay, landing |
| Executor | A subagent, one per node | Read, Edit, Write, Bash, Agent | Implement the node; start a tester and a reviewer for those stages |
| Tester | A subagent of the Executor, new for every test stage | Read, Bash | Run the tests, report what ran and what happened |
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

Each node gets a directory holding its work item and its clone:

```
.devflow/executors/<node-id>/
├── work_item.md          # the node contract, including the base commit (Orchestrator-owned)
└── <repo>/               # the clone — the only git repository in here
```

A subagent's working directory is the Orchestrator's, so the scratch directory is a location the
Executor is *told about* — by absolute path, in its work item — not one it is started in.
Keeping `work_item.md` beside the clone keeps it out of the diff under review.

---

## 3. One node, end to end

**Dispatch** — the Orchestrator, following the skill:

1. Write the node's state: `running`, Executor `working` (invariant I5 — before anything else).
2. Clone the codebase copy into the scratch directory; note the clone's commit as the **base
   commit**.
3. Write `work_item.md` from its template: the node's intent, acceptance criteria and test
   command from `graph.json`, the absolute scratch path, the base commit, and the rules.
4. Start an Executor with the work item as its prompt.

A crash between steps leaves a node marked `running` with no Executor — conservatively stale,
which recovery handles (§5).

**The loop** — the Executor:

1. **Implementation.** Make the change and its tests.
2. **Test.** Start a new tester with the clone path, the node's `tests` guidance, and the
   commands the previous tester ran, if any. It chooses what to run — at least what was run
   before — and returns pass or fail, the exact commands, their output, and whether the run
   left the tree dirty. Fail → back to 1. Third failure → `blocked`.
3. **Review.** Start a new reviewer with the clone path, the diff, the node's acceptance
   criteria, and any earlier findings. Findings → back to 1, which means testing again.
   Second round with findings → `blocked`.
4. **Confirm.** Finish, with a completion note: outcome, the commands the tester ran and their
   result, the reviewer's result, and anything the human needs.

**Completion** — the Orchestrator, when the Executor's final message arrives: record Executor
`done`, then present the diff and the completion note to the human.

Nothing mechanically proves the diff is the one that was tested and reviewed. An earlier draft
hashed the diff for exactly that, and it was removed after generated files made it misfire
(design.md §12, item 11). The Orchestrator reads the completion note instead, and sends a node
back when the tests it names do not match what the node needed.

**Revision.** The Orchestrator records Executor `working` and sends the human's feedback to the
same Executor as a message. It resumes with its context intact and goes round the loop again,
starting new testers and reviewers as before.

**Landing.** After the human lands and approves, the Orchestrator follows the skill's landing
steps (design.md §8.3): fast-forward the codebase copy, check the approved diff is present, mark
the node `complete`, remove the clone, and work out which nodes are now ready.

**Which Executor is working which node** is kept in the Orchestrator's own session, not in
`state.json`. A subagent does not outlive the session that started it, so a stored handle would
be useless after a restart.

---

## 4. Confinement

**What the MVP relies on.** The Executor's definition and its work item both say: every file
created or changed must be inside the scratch directory; never touch the Orchestrator's copies,
another node's clone, or the project root; do not commit.

**What that does not guarantee.** A subagent works in the Orchestrator's directory with the
Orchestrator's reach. Nothing but instructions stops an Executor that is confused about where
it is from writing elsewhere.

**The consequence, accepted.** An Executor that escapes can damage another node's clone, the
Orchestrator's read-only copies (invariant I8), or the project itself. That would be serious
when it happens. For an MVP it is a risk taken knowingly and addressed if it occurs; §7 lists
what to reach for.

---

## 5. Permissions and recovery

**Permissions.** All agents run inside the human's interactive session, so a permission prompt
raised by any of them reaches the human rather than being silently refused. No per-agent
permission configuration is needed.

**Recovery.** Subagents do not survive the session. When a restarted Orchestrator finds a node
`running`:

- **Executor `working`:** start a fresh Executor with the same work item. It reads the clone to
  see how far the previous one got and continues. The previous Executor's reasoning is lost;
  its output, being on disk, is not.
- **Executor `done`:** the completion note died with the session, so nothing records what was
  tested. Start a fresh Executor told the implementation is finished; it goes straight to test
  and review, then confirms. If the human had already landed and approved before the crash,
  skip to landing.

---

## 6. What is enforced, and what is written down

| What | Guaranteed by |
|---|---|
| The reviewer cannot change code | Its tool list — **enforced** |
| Tests are run by an agent other than the implementer | The loop structure — the Executor must start a tester to get a result |
| What the human sees is what was tested and reviewed | **Nothing.** Held by the Executor following its loop; see design.md §12, item 11 |
| The tester changes nothing | Its instructions; it reports a dirty tree, which makes a run that wrote into it visible |
| The tester's choice of tests is sound | Its instructions; its report lists the exact commands, and a retest must cover at least the previous round's |
| The Executor does not skip a stage or invent a report | Its instructions; the completion note must carry both reports' results, which makes a skip visible |
| Budgets: tests 3, review 2 | The Executor's instructions |
| Graph validation, ready nodes, dispatch, landing, `state.json` writes | The Orchestrator's skill, as explicit numbered steps |

Everything in the last five rows is performed by an agent following written steps. That holds up
for a handful of nodes and plainly worded procedures. Where it breaks in practice, the fix is to
move that one operation into code (design.md §12, item 11).

### The artifacts

devflow ships as a **Claude Code plugin** in its own repository. A project loads the plugin
rather than copying files into its `.claude/`, so every project runs the same version.

| File | Contents |
|---|---|
| `agents/executor.md` | The loop above, the rules, the tool list |
| `agents/tester.md` | How to choose what to run, and its report format |
| `agents/reviewer.md` | The review checklist — written out in full, since `implementation.md` does not exist inside a project — and its report format |
| `skills/devflow/SKILL.md` | The Orchestrator's phases and gates, and step-by-step procedures for validating a graph, dispatching, completing, landing, and recovering |
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
  still write anywhere, and an Executor must run tests.
- **Cost:** process lifecycle becomes devflow's job. Completion is no longer delivered to the
  Orchestrator automatically. A session id per node must be stored, which does let recovery resume
  rather than restart. Permission prompts go unanswered, so any command not allow-listed up front
  becomes an escalation.

### C. B, inside an operating-system sandbox

Run the process under `bwrap` — or `podman`, `systemd-run`, `unshare` — with only the scratch
directory writable.

- **Catches:** everything B misses, because the kernel enforces it rather than a command parser.
- **Cost:** deciding what each toolchain legitimately needs outside the scratch directory —
  `~/.cargo`, `~/.npm`, `/tmp` — and debugging builds that fail inside the sandbox but work
  outside it. The work is the list of bindings, not the wrapper.

### Considered and dropped

**Checking afterwards that only the clone changed.** Hashing everything outside the scratch
directories before and after a run detects a change but cannot attribute it to a particular
Executor, so it reports suspicion rather than fact.
