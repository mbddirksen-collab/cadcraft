//! Deterministic, offline CAM planning core.
//!
//! This crate recognizes geometric candidates and proposes editable plans. It never emits
//! machine code and never claims that a plan is collision-free or safe to run.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod adapter;
mod ai;
mod backend;
mod error;
mod planning;
mod project;
mod step;

pub use adapter::{Analysis, CadAdapter, NativeStepAdapter, ReferenceValidation, validate_references};
pub use ai::{Advisor, RuleAdvisor};
pub use backend::{CamBackend, MockCamBackend};
pub use error::{CamError, Result};
pub use planning::{PlanOptions, plan_project};
pub use project::*;
