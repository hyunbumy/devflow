---
name: devflow-exploration
description: The exploration phase of a devflow run — gather the context the goal needs, read it, and write understanding.md including open questions. Also the step that adds context later, when design finds something missing. Invoked by the devflow skill; not usually invoked directly.
---

# devflow — exploration

Getting the context the goal needs, and learning enough from it to design the work. The layout,
the state file and the standing rules are in the `devflow` skill, which sends you here.

**Scoped by the goal, not exhaustive.** You are not mapping the repository; you are learning
what the approved goal requires you to know. Stop when you could defend a design, not when
you have read everything.

## 1. Gather the context

**You cannot explore what you have not been given**, and a design built on missing context is
wrong in ways nobody notices until execution. So before reading anything, work out what the
approved goal needs and get it.

**You propose; the human decides.** Read `goal.md`, say what you think the run needs and why,
and wait. Do not choose for them — a run whose context an agent picked is a run that can
quietly omit the thing that mattered. If context already exists from an earlier session, start
from it and propose only what is missing.

Two kinds of thing, and the difference matters for the rest of the run:

| | Working codebase | Reference |
|---|---|---|
| What it is for | work will happen here | read in order to understand |
| Must be a git repository | yes | no |
| Can be a node's `repo` later | yes | **never** |

Ask about both. Codebases are usually obvious from the goal; references are the ones people
forget — the service on the other side of an interface, the spec a change has to match, a
sibling that solved the same problem already. Where a codebase's README or dependency manifest
names a neighbour that looks relevant, say so.

**Fetch only what they approved**, and tell them before you do, because it reaches the network:

```sh
git clone <source> <project>/.devflow/context/<name>     # a git repository
cp -r <source> <project>/.devflow/context/<name>         # files already on disk
```

Everything goes under `.devflow/context/<name>/`, **including the codebases that will be
changed.** Nothing is cloned for work yet — which codebases get worked on is not known until
the graph exists, and planning does that provisioning. You only read.

**Check each `codebases` entry is really a git repository.** Everything downstream needs a
branch and a commit, so a source that will not clone is a problem to raise now, while the human
is still here — not at plan approval, by which point a whole design rests on it.

Then write `context.json` at the project root:

```jsonc
{
  "codebases": [
    { "name": "api", "source": "git@github.com:acme/api.git",
      "what": "The service this work changes. Only these can be a node's repo." }
  ],
  "references": [
    { "name": "billing", "source": "git@github.com:acme/billing-service.git",
      "what": "api calls this for invoicing. Must understand it, must not change it." },
    { "name": "rfc-041", "source": "~/docs/rfc-041-auth.md",
      "what": "The spec the new auth flow has to match." }
  ]
}
```

`name` is the directory under `.devflow/context/`. `source` is where it came from, so another
checkout can re-provision the same context. `what` is one line that every later phase reads —
"the billing service" is useless; "api calls this for invoicing, must not change" is not.

**The human commits `context.json`; the material itself is gitignored.** The declaration is the
only durable record of what this run was based on.

### Coming back here later

This step runs again whenever something turns out to be missing — from further down this skill,
or from `devflow-design`. Propose the addition, fetch what is approved, update `context.json`,
and **append what you learn to `understanding.md`** rather than rewriting it.

Two limits:

- **A reference can be promoted to a working codebase** — design often concludes the
  neighbouring service has to change after all. Re-declare it under `codebases`. It must happen
  before the plan is approved, because validation rejects a node whose `repo` is not a declared
  codebase, and the graph freezes at approval.
- **Never during execution.** By then the context has done its job and everything it held is in
  `understanding.md`. A belief that turns out wrong during execution is a correction to that
  document, and possibly a blocked node — not a reason to go back and read more.

## 2. Read what the goal touches

Start from the goal and follow it into the code. Where breadth helps, spawn general-purpose
subagents to search in parallel, each with a narrow question. **Distil what they find; never
paste their output into the document.** A subagent's job is to save you reading, not to write
your understanding for you.

Read what is under `.devflow/context/` and change nothing in it. If you find yourself wanting
something that is not there, go back to step 1 rather than working around the gap.

## 3. Write `understanding.md`

At the project root:

```markdown
# Understanding

_<date>_

## Context

What `context.json` declared, and what each thing turned out to be:

- **<name>** (codebase) — <what it is; how it is built and tested>
- **<name>** (reference) — <what it is; what it settles or constrains>

## How it works today

<the architecture as it actually is, in the areas the goal touches — not as it is documented
or as it should be>

## Conventions that matter here

- <patterns a change in this area must follow>

## Constraints discovered

- <what the code forces on any solution>

## Open questions

- <what you could not settle by reading>
```

**Open questions are first-class.** An unanswered one is a reason to ask the human, never a
reason to guess. Ask them before moving on; record the answers in the document.

A question that more context would answer is not an open question — it is a trip back to
step 1. Open questions are the ones no amount of reading settles.

The file is **append-only with dated entries**. When something you wrote turns out to be
wrong later in the run, append the correction with its date rather than rewriting history —
a resumed run needs to see that a belief changed, and when.

## 4. Moving on

There is no approval gate here: exploration produces knowledge, not a commitment. When the
open questions are answered and you could defend a design:

1. Summarise for the human what you found — especially anything that contradicts what they
   assumed at intake. If the goal itself turns out to be wrong or impossible, say so and stop:
   the goal is approved and frozen, so the answer is a new run with a better goal, not a quiet
   rewrite of this one.
   Say which of the declared codebases you expect the work to change. It is not binding — the
   graph decides, and planning provisions from the graph — but it tells them what is coming.
2. Write `phase: "designing"` to `state.json`.
3. Invoke the `devflow-design` skill.
