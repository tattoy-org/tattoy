//! End to end tests

#![cfg(test)]
#![cfg(not(target_os = "windows"))]
#![expect(
    clippy::large_futures,
    reason = "
        These are just tests, and the downsides should mainfest as a showstopping stack
        overflow, so we'll know about it soon enough.
    "
)]
#![expect(
    clippy::unreadable_literal,
    clippy::dbg_macro,
    reason = "
        These are just tests
    "
)]

mod e2e;
mod gpu;
mod utils;
