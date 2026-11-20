//! AI players. Stage 11 adds the real difficulty-based AI.
//! For now this holds `RandomAttacker`, a stand-in used by tests and the Stage 7 benchmark.
//! AIs only act through `World::queue_order`, like a human.

mod random;

pub use random::RandomAttacker;
