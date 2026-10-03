//! End-to-End (E2E) Integration Test Suite for Protostellar Phase 4:
//! "Giant Molecular Clouds & Star Formation"
//!
//! Covers User Requirements:
//! - R1: High-Performance Fluid Simulation
//! - R2: Jeans Collapse and Protostar Ignition
//! - R3: Volumetric Nebula Rendering
//! - R4: Distinct Scenario Integration

#[path = "e2e_gmc_tests/harness.rs"]
pub mod harness;

#[path = "e2e_gmc_tests/tier1_feature_coverage.rs"]
pub mod tier1_feature_coverage;

#[path = "e2e_gmc_tests/tier2_boundary_corner.rs"]
pub mod tier2_boundary_corner;

#[path = "e2e_gmc_tests/tier3_cross_feature.rs"]
pub mod tier3_cross_feature;

#[path = "e2e_gmc_tests/tier4_real_world_scenarios.rs"]
pub mod tier4_real_world_scenarios;
