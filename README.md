# rust-permission-hint

An editor extension that enables **Rust permission hints**: when you hover over a
variable in a Rust file, it shows what permissions that variable currently has in
the eyes of the compiler.

The permission model comes from the ["Fixing Ownership" / ownership chapters of
_The Rust Programming Language_ (TRPL) book](https://rust-book.cs.brown.edu/),
which describe access in terms of three permissions:

- **R** — Read: the value can be read.
- **W** — Write: the value can be mutated.
- **O** — Own: the value can be moved or dropped.

Hovering over a variable (or a place expression like `*p`) reveals which of these
permissions it holds at that point in the program, making Rust's borrow-checker
rules visible and easier to learn.

## Status

Early / work in progress.
