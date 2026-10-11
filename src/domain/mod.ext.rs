// This directory's extension (hand-written; ADR-0031). The generated mod.rs (lib.rs for
// src/) pulls it in with include!, so its lines resolve against this directory.

pub mod chart_dataset;
pub mod emv_qr;
pub mod gl_posting;
pub mod reconcile_graph;
