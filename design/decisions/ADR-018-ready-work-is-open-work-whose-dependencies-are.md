+++
id = "ADR-018"
type = "decision"
title = "Ready work is open work whose dependencies are finished; milestones group it, they don't gate it"
status = "accepted"
+++

K15 adds `kiln next`, which lists the work ready to start (REQ-020).

**Work** is an item of a type the `part-of` role may link from (`task`
by default): what a milestone includes. It is **ready** when it is
neither done nor closed, its status isn't `blocked`, and every item it
`depends-on` is done or closed. Everything else open is **waiting**,
with what it waits on named.

**A milestone groups and orders work, it doesn't gate it.** Work of a
milestone that `depends-on` an unfinished one is still ready when its
own dependencies are: teams overlap milestones, and a gate would hide
the work an agent could pick up. `kiln next` lists the earliest
milestone's work first (open milestones in ID order, work in no
milestone last), so the order the IDs give is the order an agent sees.
The cost: a project that does want a gate writes it, `depends-on` from
the first task of the later milestone to the last of the earlier one.

**`blocked` is a status a person sets, so it is honoured** by name, like
the other default status names some rules know (`verified`,
`superseded`): a project that names it otherwise simply doesn't get
that.

The same definitions feed `kiln status` and the plan checks K211–K214,
so what `next` calls ready is what `check` and `status` agree on.
