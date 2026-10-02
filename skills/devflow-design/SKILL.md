---
name: devflow-design
description: The design phase of a devflow run — propose how the goal will be met, iterate with the human, and get design.md approved before planning. Invoked by the devflow skill; not usually invoked directly.
---

# devflow — design

Agreeing *how* the goal will be met, before it is broken into work. The layout, the state
file and the standing rules are in the `devflow` skill, which sends you here.

You are designing, not planning: this says what changes and why, not who does what in which
order. Node-sized decisions belong to the next phase.

## 1. Propose

Work from `goal.md` and `understanding.md`. Propose one approach, and say what you rejected.

A design is finished when it answers all of these:

- **The approach** — what changes, and why this way.
- **Alternatives rejected** — what else you considered, and the reason it lost. A design with
  no rejected alternative usually means you took the first idea.
- **Components affected** — which codebases and which parts of them.
- **Interfaces** — anything new or changed at a boundary: signatures, schemas, endpoints,
  file formats.
- **Non-goals** — what this design deliberately does not do, so a later reader does not
  mistake an omission for an oversight.

Where the design rests on something you could not settle during exploration, say so in the
document rather than deciding quietly.

**If you need context nobody gathered, go and get it.** Designing often reveals that the service
on the other side of an interface, or a spec the change has to match, should have been read.
Invoke `devflow-exploration` — its first step adds context — and append what you learn to
`understanding.md`. This is also where a reference becomes a working codebase, if the design
concludes that something declared read-only has to change after all. Do it now: once the plan is
approved the graph is frozen, and a node cannot name a codebase that was never declared.

If designing shows the approved goal to be wrong, stop and say so. The goal is frozen; this run
ends and the next one starts from a goal that fits what you now know. Do not design for a goal
the human did not approve.

## 2. Iterate

Show it, take the human's objections seriously, and rewrite. Push back when you disagree —
they are relying on your judgement, not your agreement — but their decision settles it once
they have the facts.

Write it to `design.md` at the project root:

```markdown
# Design

## Approach

<what changes, and why this way>

## Alternatives rejected

- **<the alternative>** — <why it lost>

## Components affected

- **<codebase>** — <what changes in it>

## Interfaces

- <new or changed signature, schema, endpoint, format>

## Non-goals

- <what this design deliberately leaves alone>
```

## 3. The gate

**Do not move on without an explicit yes.** Ask plainly whether you should turn this into a
work graph.

On approval:

1. Write `phase: "planning"` to `state.json`.
2. Invoke the `devflow-planning` skill.

The human commits `design.md` when they choose; you never commit anything.
