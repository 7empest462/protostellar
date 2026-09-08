//! Protostellar — Scientifically accurate solar system formation simulator in Rust.
#![allow(
    clippy::type_complexity,
    clippy::trivially_copy_pass_by_ref,
    clippy::explicit_auto_deref,
    clippy::chunks_exact_to_as_chunks
)]

pub mod game;
pub mod gpu;
pub mod rendering;
pub mod simulation;
pub mod utils;
