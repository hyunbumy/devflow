---
name: devflow-intake
description: The intake phase of a devflow run — restate what the human wants as goal.md and get their explicit approval before any exploration begins. Invoked by the devflow skill; not usually invoked directly.
---

# devflow — intake

Agreeing what this run is for, before a single token goes into reading code. The layout, the
state file and the standing rules are in the `devflow` skill, which sends you here.

This phase is cheap and catches the most expensive mistake there is: building the wrong
thing. Do not rush it, and do not start exploring to answer questions you could simply ask.

## 1. Listen, then say it back

Ask what they want done. Then restate it **in your own words** — not a summary of their
phrasing, but what you understand the work to be. Misunderstandings surface here or not at
all.

Ask about anything the request leaves open, one thing at a time:

- **What does done look like?** Something checkable at the end of the run.
- **What must not change?** Compatibility, systems to leave alone, deadlines.
- **What is out of scope?** The work adjacent to this that you are *not* doing.

If they say "you decide", say what you would decide and let them correct it. A silent
assumption at intake becomes a wrong graph later.

## 2. Write `goal.md`

At the project root, short enough to read in a minute:

```markdown
# Goal

<the request, in your own words — a paragraph, not a transcript>

## Done looks like

- <something checkable when the run finishes>

## Constraints

- <what must not change, and why>

## Out of scope

- <adjacent work this run will not do>
```

Show it to them and take corrections. Rewrite it until they recognise their own intent in
it.

## 3. The gate

**Do not move on without an explicit yes.** "Looks fine" is not approval; ask plainly whether
you should start exploring.

On approval:

1. Write `state.json` with `phase: "exploring"` and no nodes yet, creating the file if this
   is a new run.
2. Tell them exploration is starting, and invoke the `devflow-exploration` skill.

The human commits `goal.md` when they choose; you never commit anything.
