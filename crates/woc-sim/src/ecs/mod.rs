//! Typed sparse-column ECS for the deterministic sim.
//!
//! Not Bevy. Iteration order is spawn/insertion order.

pub mod sparse;

pub use sparse::SparseSet;
