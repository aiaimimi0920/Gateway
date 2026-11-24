//! Fast-compiling provider SDK & declarative descriptor kit.
//!
//! New providers declare their origin policy, authentication mode, endpoint capabilities,
//! and console UI field schema once via [`descriptor::ProviderDescriptor`], and reuse
//! bounded SSE/wire primitives from [`sse`] without recompiling the full Gateway crate.

pub mod descriptor;
pub mod sse;

#[cfg(test)]
mod tests;
