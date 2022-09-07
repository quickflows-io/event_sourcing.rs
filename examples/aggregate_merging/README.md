This example demonstrates a `Projector` which consumes events from two different `Aggregates`, and projects those events
to a single projection (in this case, a count of how many of each command the aggregates have received)

When either `AggregateA` or `AggregateB` in this example receive a command, they translate that into an (empty) `Event`
specific to them (`EventA` or `EventB`). Both of these implement `Into<ProjectorEvent>`, and as such, both `AggregateA`
and `AggregateB` can have an instantiation of `CounterProjector` in their projectors vec. This aggregate specific event
is passed, by the underlying `EventStore`, to the aggregates instantiation of the generic `CounterProjector` (
either `CounterProjector<EventA>` or `CounterProjector<EventB>`), and this projector updates the event counts in the
projection. NOTE: this is race prone. See projector.rs for more details about how to make this sound.