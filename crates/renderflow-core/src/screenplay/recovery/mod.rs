//! Explicitly lossy screenplay recovery with immutable extraction and review evidence.
//!
//! The deterministic candidate is never an approved original. Native extraction
//! requires explicit local pins; no provider installation or AI fallback occurs.

pub mod model;
pub mod provider;
pub mod export;
pub mod graph;
