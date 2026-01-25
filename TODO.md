in the case of e.g. `#[effect(write_cache)]`,
it wraps a function typically (should be enforced by `write_cache` being used, either
that it wraps a function or a block expression/statement).

This emits two instructions under the effects system - a `begin_write_cache` event
and then an `end_write_cache` event. Probably generalized to simply `begin(write_cache)` and
`end(write_cache)` with some sort of token, and the effect system checks that all begins/ends
are balanced.

Anyway, during the `write_cache` segment, a register is going to be read. This might need to be
enforced somehow (e.g. otherwise it'd error with `write_cache didn't check any processor state; it's useless`
or something).

It then tracks what exactly was referenced, and then any further effects that change the referenced
state (e.g. the `cr4` register, perhaps down to the bit level) will then be stored in the state.
This state is then checked such that any further writes to the referred-to registers followed by
a read from the cache (where a write to that cache hasn't been performed between the source state
update and the cache read) will error with e.g.
`cache read was stale; write to cr4 occurred at <pos> but was stale at <pos>` or something.

This is a runtime validation system that will ultimately be used to interop with either QEMU
via a serial port (or perhaps a custom device driver of some sort) and over the serial line
for baremetal if I adapt the system to that. It'll be an external rust daemon that will
keep track of this stuff.

I want to get this in there early in order to catch weird logic bugs.

These will take the form of performing an extern import of some function and calling it with
various different strings etc. that are emitted into a special section and used to debug just
IDs similar to defmt.

Completely disabled and stripped in release builds of course.