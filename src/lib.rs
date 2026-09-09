//! Protostellar — Scientifically accurate solar system formation simulator in Rust.

pub mod game;
pub mod gpu;
pub mod rendering;
pub mod simulation;
pub mod utils;
// Referenced for Cargo feature / fast-math linkage; silence unused-dep scanners.
use glam as _;
