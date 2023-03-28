# Tracing Span Dump

A work in progress tracing-subscriber layer capable of tracking what the current state of your spans are. The idea is this can be supplementary information for debugging (since thread dumps in async programs maybe hard to read).

## Challenges of spans & futures

The issue with trying to understand running futures is that they are by design, often not running. To cope with this tracing-span-dump keeps records of all spans that have not been dropped yet.

## Other similar crates & solutions
- https://crates.io/crates/async-backtrace
- https://github.com/tokio-rs/console