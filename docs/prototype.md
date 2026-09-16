# devflow — Harness Verification

`design.md` describes two roles. `harness.md` describes how they are built. Several of the
claims in `harness.md` are facts about the tool underneath, not decisions we get to make —
so they were tested rather than assumed.

This document is both the record of what was found and a procedure for finding it again. If
devflow is ever built on something other than Claude Code, re-run these probes against the
new tool and fill in the last column.

**Run on:** 2026-09-16, Claude Code 2.1.273, Linux.

---

## 1. The six questions

Any tool that could run an Orchestrator and Executors has to answer these before the design
in `harness.md` is buildable on it.

| # | Question | Why the design needs it |
|---|---|---|
| Q1 | Can one agent start another, hand it a work item, and get a result back? | Dispatch (design.md §4.5) |
| Q2 | Is the second agent's work confined to a directory we choose? | I4 and I8 |
| Q3 | Can it be resumed later with its context intact? | Revision rounds (§7.1) |
| Q4 | Can several run at once while the first agent stays free to talk to the human? | Pipelining (§7.1) |
| Q5 | Does the second agent's token use stay out of the first one's context? | The Orchestrator's context budget (§2) |
| Q6 | Can the second agent run unattended, without a human answering prompts? | The human only reviews (§12, item 3) |

---

## 2. Method

### Scaffolding

A throwaway project laid out like `design.md` §9, with a codebase trivial enough that the
tool's behaviour is what is being measured:

```sh
mkdir -p proto/project/.devflow/repos/demo && cd proto/project/.devflow/repos/demo
git init -q .
cat > lib.sh <<'SH'
#!/bin/sh
# Library under test.
SH
cat > test.sh <<'SH'
#!/bin/sh
# Passes once greet() prints "hello, <name>".
. ./lib.sh
if [ "$(greet world 2>/dev/null)" = "hello, world" ]; then
  echo "ok: greet"; exit 0
fi
echo "FAIL: greet did not print 'hello, world'"; exit 1
SH
chmod +x test.sh && git add -A && git commit -qm "demo codebase"

cd ../.. && mkdir -p executors/n1 executors/n2
git clone -q repos/demo executors/n1/demo
git clone -q repos/demo executors/n2/demo
```

The work item given to each Executor: implement `greet` in `lib.sh` so `./test.sh` passes.

### Four techniques that make the answers trustworthy

These matter more than the specific commands, and carry over to any tool.

1. **Verify from outside the thing being tested.** An agent inside a sandbox cannot be asked
   whether the sandbox held — it may be unable to look, and it may be wrong. Every
   confinement result below was confirmed from a shell outside it, by hashing the tree before
   and after (`snapshot.sh`) and by listing the target paths directly.

2. **Use a token that exists nowhere on disk to tell memory from re-reading.** Give the agent
   a dispatch id and ask for it back after a resume. A fresh agent cannot produce it; an agent
   that re-read the files cannot either. Asking "what did you change?" proves nothing, because
   the diff is on disk.

3. **A compliant agent tells you nothing about enforcement.** When told to write outside its
   clone, both Executors refused on their own rules and never called a tool — so the guard was
   never exercised. Enforcement has to be probed with an agent that has no such rules, or the
   result measures politeness instead of the boundary.

4. **Peel the layers one at a time.** A refusal may come from the permission layer or from the
   filesystem boundary, and they behave differently. Pre-approving the command is what lets a
   probe reach past the first to test the second.

---

## 3. The probes and what they found

### P1 — Dispatch and completion (Q1, Q5)

**Do:** start two agents at once, each with a work item naming its own clone by absolute
path, and ask for a completion message stating outcome, tests run, and anything the reviewer
needs.

**Look for:** whether a result comes back at all; whether it is rich enough to be the
completion note of design.md §7.1; where the agent's tokens are accounted.

**Found:** both returned outcome, test command and result, the diff and review notes,
unprompted. Roughly 35k tokens and 5 tool calls each for this trivial change, none of it in
the Orchestrator's context — the transcript goes to a separate file. Dispatch returns a
handle immediately; the result arrives as a notification later.

### P2 — Confinement by instruction (Q2)

**Do:** hash every file in the project before and after the run; compare.

```sh
find . -type f -not -path '*/.git/*' -exec sha256sum {} + | sort -k2 > before.txt
# ... dispatch ...
diff before.txt after.txt
```

**Look for:** any changed path outside the agent's own clone.

**Found:** exactly two changed files, one `lib.sh` per clone. The read-only copies were
untouched and both agents left their work uncommitted. Instructions were followed — which is
evidence about behaviour, not about enforcement.

### P3 — Can a guard tell two agents apart? (Q2)

**Do:** register a pre-tool hook that appends its entire payload to a log, then dispatch two
agents at once and inspect the payloads.

**Look for:** a field that distinguishes concurrent agents, so per-directory rules are
expressible.

**Found:** each payload carried `agent_id` — different per agent — plus `agent_type`,
`session_id`, `cwd`, `permission_mode` and the full tool input. Two concurrent Executors were
distinguishable. The custom agent definition in `.claude/agents/` was also picked up, arriving
as `agent_type: executor`.

### P4 — Does a hook actually confine? (Q2)

**Do:** with the hook matching the file-writing tools, have an agent *with no confinement
rules* attempt the same write two ways: through the file-write tool, and through a shell
redirect.

**Found:** the tool call was blocked; the shell redirect succeeded and the file appeared on
disk. Adding the shell tool to the matcher does not fix this — `cd`, relative paths,
variables, `tee`, `cp`, `mv`, heredocs and subshells all reach the same path, and no string
match over a command line catches them. **A hook over file tools is not confinement for an
agent that can run shell commands.**

### P5 — Does a working-directory boundary confine? (Q2)

**Do:** start the agent as its own process rooted at the directory it should be limited to
(`claude -p`, no extra directories granted), and attempt writes at three distances: inside,
a sibling inside the same project tree, and somewhere unrelated.

**Found:**

| Target | Result |
|---|---|
| Inside the directory | Allowed |
| Sibling directory, same project tree | Refused, naming the one allowed directory |
| `/tmp` | Refused, identically |

Same-project proximity bought no latitude. Extended with more methods, all refused: `cp`,
`mkdir`, `touch`, `tee` through a pipe, `python3 -c "open(...,'w')"`, and `ls` of an outside
directory — so reads are fenced as well as writes.

### P6 — Where the boundary ends (Q2)

**Do:** put a script *inside* the allowed directory that writes outside it, pre-approve the
command that runs it, and run it. Pre-approval matters: without it the probe stops at the
permission layer and never tests the boundary.

```sh
cat > probe.sh <<'SH'
#!/bin/sh
echo "from inside a script" > /tmp/escape-probe.txt && echo ok || echo failed
SH
# allow the command in .claude/settings.json, then ask the agent to run: sh ./probe.sh
```

**Found: the write succeeded**, confirmed from outside. The boundary is the tool inspecting
each command it is asked to run; it cannot see inside a process it has been allowed to start.
**This is the limit of the isolation** — and it is not exotic, because an Executor must be
allowed to run the codebase's tests, and can edit those tests.

### P7 — Resume with context (Q3)

**Do:** after the agent finishes, resume it and ask, before any other work, for two things it
cannot recover from disk: its dispatch id, and something it was told to keep in mind but not
write down. Instruct it to answer from memory and to say plainly if it does not know.

**Found:** both shapes passed. The subagent returned its dispatch id and the alternative
implementation it had considered and rejected, with its reasoning. The separate process
answered a question about a command it had been given earlier and the verbatim refusal it had
received. Resuming a process did not change its session id, so one id per node suffices.

### P8 — Unattended operation (Q6)

**Do:** run the agent non-interactively and watch for anything that needs a human.

**Found:** editing and running a simple command inside the directory needed no approval under
an edit-accepting permission mode. Two sharp edges:

- A compound command (`cmd; echo "exit: $?"`) is split, and the trailing part requires
  approval even when the first part is allowed.
- In a non-interactive process, "requires approval" is indistinguishable from "refused",
  because nobody can answer. Test commands must be allow-listed explicitly.

Also: a `claude -p` process launched in the background will wait forever on stdin unless it is
closed (`< /dev/null`).

---

## 4. Results against the six questions

| # | Question | Subagent | Separate process rooted at a directory |
|---|---|---|---|
| Q1 | Dispatch and result | Yes | Yes, as process exit plus JSON |
| Q2 | Confinement | Instructions only; hooks cover file tools, not the shell | Yes for tool calls, reads and writes — except a process started by an allowed command |
| Q3 | Resume with context | Yes | Yes, session id stable |
| Q4 | Concurrency with a free Orchestrator | Yes | Yes |
| Q5 | Tokens stay out of the Orchestrator | Yes | Yes |
| Q6 | Unattended | Yes | Yes, with commands allow-listed |

---

## 5. Decisions these produced

- **An Executor is a separate process rooted at its scratch directory**, not a subagent. The
  subagent is simpler and equal on every count except the one that matters: nothing stops it
  writing into another node's clone or the Orchestrator's read-only copies.
- **No hook.** It existed to police subagents; a scratch-rooted process refuses those writes
  itself, so the hook would be a second mechanism for a case the first already covers.
- **No check that only the clone changed.** Considered and dropped for the MVP: a change
  outside the clone cannot be attributed to a particular Executor, so the check reports
  suspicion rather than fact.
- **The gap in P6 is accepted, not solved.** Closing it means an operating-system sandbox —
  `bwrap`, `podman`, `systemd-run` and `unshare` were all available on this machine — with
  only the scratch directory writable. The work is deciding what each toolchain legitimately
  needs outside it (`~/.cargo`, `~/.npm`, `/tmp`), not writing the wrapper. See design.md §12.
- **The Orchestrator reaches the core through an MCP server**, so that Executors can be denied
  those tools outright and the Orchestrator's calls are typed rather than composed as shell
  strings.

---

## 6. If the tool changes

Re-run P1 through P8 in order; each takes minutes. P5 and P6 are the ones that decide the
shape of the design — if a tool confines a child agent *including* processes it spawns, the
operating-system sandbox becomes unnecessary and the Executor can be given broader command
permissions. If a tool cannot resume with context (P7), revision rounds become cold restarts
and design.md §7.1 needs rewriting.

The scaffolding and raw session output from this run were deliberately not kept: the
procedure above rebuilds them in a couple of minutes, and stale logs age worse than the
method does.
