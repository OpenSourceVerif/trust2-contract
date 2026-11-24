//! Compiler facts adapted to the final ULLBC after normal transformations.
//! All public locations/locals are ULLBC identities. Compiler region numbers
//! inhabit a separate namespace from `Region::Body`.
pub const PRE_STRUCTURE_BORROWCK_STAGE: &str = "ullbc-pre-structure-borrowck-v2";
pub const FINAL_ULLBC_BORROWCK_STAGE: &str = "ullbc-final-borrowck-v2";

use crate::ast::*;
use crate::ullbc_ast::{BlockId, ExprBody};
use derive_generic_visitor::{Drive, DriveMut, DriveTwo};
use serde::{Deserialize, Serialize};
use serde_state::{DeserializeState, SerializeState};

#[derive(
    Clone,
    Debug,
    Serialize,
    Deserialize,
    SerializeState,
    DeserializeState,
    Drive,
    DriveMut,
    DriveTwo,
)]
#[serde_state(stateless)]
pub struct BorrowckData {
    pub schema_version: usize,
    pub producer: String,
    pub stage: String,
    pub bodies: Vec<BodyBorrowckInfo>,
}
#[derive(
    Clone,
    Debug,
    Serialize,
    Deserialize,
    SerializeState,
    DeserializeState,
    Drive,
    DriveMut,
    DriveTwo,
)]
#[serde_state(stateless)]
pub struct BodyBorrowckInfo {
    pub function_id: FunDeclId,
    /// Explicit reason when the compiler adapter cannot represent this body.
    pub unsupported: Option<String>,
    /// Detect accidental use with a body modified after extraction.
    pub body_fingerprint: String,
    pub loans: Vec<BorrowckLoan>,
    pub points: Vec<BorrowckPoint>,
    pub reference_regions: Vec<BorrowckReferenceRegion>,
    /// Evaluated NLL outlives relation, restricted to the exported regions.
    pub outlives: Vec<BorrowckOutlives>,
}
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    SerializeState,
    DeserializeState,
    Drive,
    DriveMut,
    DriveTwo,
)]
#[serde_state(stateless)]
pub struct BorrowckLocation {
    pub block: BlockId,
    pub statement_index: usize,
}
#[derive(
    Clone,
    Debug,
    Serialize,
    Deserialize,
    SerializeState,
    DeserializeState,
    Drive,
    DriveMut,
    DriveTwo,
)]
#[serde_state(stateless)]
pub struct BorrowckLoan {
    pub id: usize,
    pub reserve: BorrowckLocation,
    pub borrowed: LocalId,
    pub assigned: LocalId,
    pub is_mutable: bool,
    pub region: usize,
}
#[derive(
    Clone,
    Debug,
    Serialize,
    Deserialize,
    SerializeState,
    DeserializeState,
    Drive,
    DriveMut,
    DriveTwo,
)]
#[serde_state(stateless)]
pub struct BorrowckPoint {
    pub location: BorrowckLocation,
    /// True for inserted operations with no loan effect (storage markers and
    /// unit initialization). Both sets equal the before-state at the insertion
    /// boundary; locations always name the final ULLBC.
    pub synthetic: bool,
    /// rustc Borrows state before/after the primary effect, NOT PCG phases.
    pub before: Vec<usize>,
    pub after: Vec<usize>,
}
#[derive(
    Clone,
    Debug,
    Serialize,
    Deserialize,
    SerializeState,
    DeserializeState,
    Drive,
    DriveMut,
    DriveTwo,
)]
#[serde_state(stateless)]
pub struct BorrowckReferenceRegion {
    pub local: LocalId,
    pub region: usize,
}
#[derive(
    Clone,
    Debug,
    Serialize,
    Deserialize,
    SerializeState,
    DeserializeState,
    Drive,
    DriveMut,
    DriveTwo,
)]
#[serde_state(stateless)]
pub struct BorrowckOutlives {
    pub longer: usize,
    pub shorter: usize,
}

/// Versioned, deterministic FNV-1a checksum of the complete, non-deduplicated
/// serialized body. This is an accidental-staleness check, not authentication.
pub fn borrowck_body_fingerprint(body: &ExprBody) -> String {
    let mut bytes = Vec::new();
    body.serialize_state(&(), &mut serde_json::Serializer::new(&mut bytes))
        .expect("serializing a ULLBC body to memory");
    let hash = bytes.iter().fold(0xcbf29ce484222325u64, |hash, b| {
        (hash ^ u64::from(*b)).wrapping_mul(0x100000001b3)
    });
    format!("fnv1a64-v1:{hash:016x}")
}
