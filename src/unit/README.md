# cooper-unit

This crate is a very simple inernally used test framework that features
global and per-test setup and teardown. It is based somewhat on the
`libtest` that comes with Rust.

## Advantages

- Setup and teardown
- Expected failure
- Single threaded

## Disadvantages

- No automatic "test discovery" (i.e. codegen)
- Hand-rolling stuff is unfortunate
- Single threaded
