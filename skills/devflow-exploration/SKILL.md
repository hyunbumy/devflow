---
name: devflow-exploration
description: The exploration phase of a devflow run — read the codebases the goal touches and write understanding.md, including open questions. Invoked by the devflow skill; not usually invoked directly.
---

# devflow — exploration

Learning enough about the codebases to design the work. The layout, the state file and the
standing rules are in the `devflow` skill, which sends you here.

**Scoped by the goal, not exhaustive.** You are not mapping the repository; you are learning
what the approved goal requires you to know. Stop when you could defend a design, not when
you have read everything.

## 1. Inventory the codebases

For each directory in `.devflow/repos/`: what it is, what it is for, and how it is built and
tested. That list is the first thing `understanding.md` needs — later phases depend on knowing
which codebases exist and what each one holds.

## 2. Read what the goal touches

Start from the goal and follow it into the code. Where breadth helps, spawn general-purpose
subagents to search in parallel, each with a narrow question. **Distil what they find; never
paste their output into the document.** A subagent's job is to save you reading, not to write
your understanding for you.

Read the codebase copies under `.devflow/repos/` and change nothing in them.

## 3. Write `understanding.md`

At the project root:

```markdown
# Understanding

_<date>_

## Codebases

- **<name>** — <what it is; how it is built and tested>

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
2. Write `phase: "designing"` to `state.json`.
3. Invoke the `devflow-design` skill.
