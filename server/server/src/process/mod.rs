//! How this server starts other processes.
//!
//! One rule, and it is a Windows rule: a child spawned by a process that has no
//! console gets a console of its own, and that console is a window on somebody's
//! desktop. Everything that spawns goes through `Windowless`.

mod windowless;

pub use windowless::Windowless;
