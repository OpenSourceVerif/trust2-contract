//! Pre-borrowck specification extraction, host MIR cleanup, and anchor legality checking.
//!
//! `trust2-contract` lowers surface specifications (`#[precondition]`, `#[postcondition]`,
//! `#[invariant]`, `contract_assert!`, `contract_assume!`, `forall!`, `exists!`) into closures
//! passed to `trust2_contract::internal::*` helpers so `rustc` performs name resolution, type
//! checking, and closure capture analysis.
//!
//! Leaving those helper calls and closure captures in the host MIR causes two problems:
//! 1. Specifications consume non-`Copy` values (`E0382`) or conflict with host borrows (`E0502`),
//!    polluting `mir_borrowck` facts (`ullbc-pcg`) and `--precise-drops` drop elaboration.
//! 2. Specification bodies themselves are pure logical formulas and should allow multiple by-value
//!    uses of non-`Copy` values or projections out of shared references without runtime moves.
//!
//! This module intercepts `mir_built` and `mir_promoted` via `override_queries`:
//! - For **host bodies**, `mir_built` records each specification closure and its captured places,
//!   then erases the closure construction and helper call (`Goto` for function contracts; a
//!   zero-capture `entry(anchor_id)` marker for inline assertions).
//! - For **specification bodies** (spec closures, transitively nested quantifier closures, and
//!   `TypeInvariant::invariant` implementations), `mir_built` validates pure-spec structural rules
//!   (no loops, no `&mut` borrows, no projected writes, no explicit `drop` calls) and strips all
//!   implicit runtime drops; before `mir_borrowck` runs, `prepare_crate_specs` snapshots their
//!   `mir_built` bodies for Charon while `mir_promoted` substitutes a trivial self-loop so
//!   `mir_borrowck` and drop elaboration ignore linear move constraints inside specifications.
//! - After `mir_borrowck` runs on the cleaned host bodies, `prepare_crate_specs` checks at every
//!   anchor point (via `MoveData`, `MaybeUninitializedPlaces`, `Borrows`, and `places_conflict`)
//!   that captured host places are initialized, not moved, and not frozen by an active `&mut`.

use crate::hax::Spanned;
use charon_lib::ast::ContractAssertKind;
use charon_lib::options::CliOpts;
use rustc_abi::{FieldIdx, Size, VariantIdx};
use rustc_borrowck::consumers::{
    BodyWithBorrowckFacts, Borrows, ConsumerOptions, PlaceConflictBias,
    get_bodies_with_borrowck_facts, places_conflict,
};
use rustc_const_eval::interpret::Scalar;
use rustc_data_structures::steal::Steal;
use rustc_hash::{FxHashMap, FxHashSet};
use rustc_hir::LangItem;
use rustc_hir::def::DefKind;
use rustc_hir::def_id::{DefId, LocalDefId};
use rustc_hir::definitions::DefPathData;
use rustc_index::IndexVec;
use rustc_index::bit_set::DenseBitSet;
use rustc_middle::mir::{
    self, AggregateKind, BasicBlock, BasicBlockData, Body, BorrowKind, Const, ConstOperand,
    ConstValue, Local, Location, Operand, Place, PlaceElem, PlaceTy, ProjectionElem, Rvalue,
    SourceInfo, StatementKind, Terminator, TerminatorKind,
};
use rustc_middle::ty::{self, TyCtxt};
use rustc_middle::util::Providers;
use rustc_mir_dataflow::{
    Analysis,
    impls::MaybeUninitializedPlaces,
    move_paths::{LookupResult, MoveData},
    on_all_children_bits,
};
use rustc_span::{DUMMY_SP, Span, sym};
use std::num::NonZeroU32;
use std::sync::{LazyLock, Mutex};

/// Global per-driver state shared with `rustc`'s `override_queries` function pointers.
#[derive(Default)]
struct GlobalSpecState {
    /// Whether this compiler invocation translates the crate into ULLBC (`true`) or only erases
    /// specifications for a normal dependency build (`false`).
    record_host_sites: bool,
    /// Local definitions whose bodies are pure logical specifications.
    spec_bodies: FxHashSet<LocalDefId>,
    /// Extracted specification sites per host body owner, drained by `prepare_crate_specs`.
    host_sites: FxHashMap<LocalDefId, Vec<HostSpecSite>>,
}

static STATE: LazyLock<Mutex<GlobalSpecState>> =
    LazyLock::new(|| Mutex::new(GlobalSpecState::default()));

/// Clears global specification state before or after a compiler invocation.
pub(crate) fn reset_state() {
    let mut state = STATE.lock().unwrap();
    state.record_host_sites = false;
    state.spec_bodies.clear();
    state.host_sites.clear();
}

/// Configures whether query hooks should retain `HostSpecSite` metadata for Charon translation.
pub(crate) fn configure_session(record_host_sites: bool) {
    let mut state = STATE.lock().unwrap();
    state.record_host_sites = record_host_sites;
    state.spec_bodies.clear();
    state.host_sites.clear();
}

/// Installs `mir_built`, `mir_promoted`, and `check_liveness` query hooks into `providers`.
pub(crate) fn install_query_overrides(providers: &mut Providers) {
    providers.queries.mir_built = mir_built_hook;
    providers.queries.mir_promoted = mir_promoted_hook;
    providers.queries.check_liveness = check_liveness_hook;
}

/// Prepared MIR artifacts extracted before borrow checking and drop elaboration.
pub(crate) struct PreparedSpecMir<'tcx> {
    /// Pre-borrowck MIR bodies for all specification closures and `TypeInvariant` methods.
    pub spec_mir_bodies: FxHashMap<LocalDefId, Body<'tcx>>,
    /// Cached `BodyWithBorrowckFacts` computed while validating host specification anchors.
    pub borrowck_bodies: FxHashMap<LocalDefId, BodyWithBorrowckFacts<'tcx>>,
    /// Extracted host specification sites per host body owner.
    pub host_sites: FxHashMap<LocalDefId, Vec<HostSpecSite>>,
}

/// Kind of `trust2_contract::internal` helper call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SpecCallKind {
    Entry,
    Precondition,
    Postcondition,
    Assert,
    Assume,
    ForAll,
    Exists,
}

/// Kind of host-level specification attached to a function or program point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HostSpecKind {
    /// Function precondition (`#[precondition(...)]`).
    Precondition,
    /// Function postcondition (`#[postcondition(...)]`).
    Postcondition,
    /// Inline assertion or assumption (`contract_assert!` / `contract_assume!`) anchored at an
    /// `internal::entry(anchor_id)` call.
    InlineAssert {
        /// Assertion vs assumption kind.
        kind: ContractAssertKind,
        /// 1-based anchor identifier encoded in `internal::entry(anchor_id)`.
        anchor_id: NonZeroU32,
    },
}

/// How a host place was captured by a specification closure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CaptureMode {
    /// Captured by shared borrow (`&place`).
    SharedRef,
    /// Captured directly by value (`move place` / `copy place`).
    ByValue,
}

/// Lifetime-erased representation of a `mir::ProjectionElem` for `'static` storage.
#[derive(Clone, Copy, Debug)]
pub(crate) enum SavedProjElem {
    /// Pointer/reference dereference (`*p`).
    Deref,
    /// Struct/tuple/enum-variant field projection (`p.idx`).
    Field(FieldIdx),
    /// Enum variant downcast (`(p as Variant)`).
    Downcast(VariantIdx),
    /// Dynamic slice/array index (`p[idx_local]`).
    Index(Local),
    /// Constant slice/array index.
    ConstantIndex {
        /// Index offset.
        offset: u64,
        /// Minimum slice length.
        min_length: u64,
        /// Whether `offset` counts from the end.
        from_end: bool,
    },
}

/// Lifetime-erased representation of a host `mir::Place<'tcx>`.
#[derive(Clone, Debug)]
pub(crate) struct SavedPlace {
    /// Root local variable in the host MIR body.
    pub local: Local,
    /// Sequence of projections applied to `local`.
    pub projection: Box<[SavedProjElem]>,
}

impl SavedPlace {
    /// Encodes `place` into a `'static` representation, returning `None` if an unsupported
    /// projection kind is encountered.
    fn from_mir(place: Place<'_>) -> Option<Self> {
        let mut projection = Vec::with_capacity(place.projection.len());
        for elem in place.projection {
            let saved = match elem {
                ProjectionElem::Deref => SavedProjElem::Deref,
                ProjectionElem::Field(idx, _) => SavedProjElem::Field(idx),
                ProjectionElem::Downcast(_, variant) => SavedProjElem::Downcast(variant),
                ProjectionElem::Index(local) => SavedProjElem::Index(local),
                ProjectionElem::ConstantIndex {
                    offset,
                    min_length,
                    from_end,
                } => SavedProjElem::ConstantIndex {
                    offset,
                    min_length,
                    from_end,
                },
                ProjectionElem::Subslice { .. }
                | ProjectionElem::OpaqueCast(_)
                | ProjectionElem::UnwrapUnsafeBinder(_) => return None,
            };
            projection.push(saved);
        }
        Some(Self {
            local: place.local,
            projection: projection.into_boxed_slice(),
        })
    }

    /// Reconstructs a typed `mir::Place<'tcx>` against `body.local_decls`.
    pub(crate) fn to_mir_place<'tcx>(&self, tcx: TyCtxt<'tcx>, body: &Body<'tcx>) -> Place<'tcx> {
        let typing_env = ty::TypingEnv::non_body_analysis(tcx, body.source.def_id());
        let mut place_ty = Place::from(self.local).ty(&body.local_decls, tcx);
        let mut elems: Vec<PlaceElem<'tcx>> = Vec::with_capacity(self.projection.len());
        for &saved in &self.projection {
            let elem = match saved {
                SavedProjElem::Deref => ProjectionElem::Deref,
                SavedProjElem::Field(idx) => {
                    let raw_fty = PlaceTy::field_ty(tcx, place_ty.ty, place_ty.variant_index, idx);
                    let fty = tcx
                        .try_normalize_erasing_regions(typing_env, raw_fty)
                        .unwrap_or_else(|_| raw_fty.skip_normalization());
                    ProjectionElem::Field(idx, fty)
                }
                SavedProjElem::Downcast(variant) => ProjectionElem::Downcast(None, variant),
                SavedProjElem::Index(local) => ProjectionElem::Index(local),
                SavedProjElem::ConstantIndex {
                    offset,
                    min_length,
                    from_end,
                } => ProjectionElem::ConstantIndex {
                    offset,
                    min_length,
                    from_end,
                },
            };
            place_ty = place_ty.projection_ty(tcx, elem);
            elems.push(elem);
        }
        Place {
            local: self.local,
            projection: tcx.mk_place_elems(&elems),
        }
    }
}

/// One captured host place recorded from an erased specification closure.
#[derive(Clone, Debug)]
pub(crate) struct SavedCapture {
    /// Capture mode (shared reference vs by-value).
    pub mode: CaptureMode,
    /// Host place read by the specification.
    pub place: SavedPlace,
    /// Source span of the capture for diagnostics.
    pub span: Span,
}

/// Metadata recorded for a single specification site inside a host MIR body.
#[derive(Clone, Debug)]
pub(crate) struct HostSpecSite {
    /// Specification kind (precondition, postcondition, or anchored inline assert/assume).
    pub kind: HostSpecKind,
    /// Local `DefId` of the specification closure.
    pub closure_def_id: LocalDefId,
    /// Captured host places in closure upvar field order.
    pub captures: Box<[SavedCapture]>,
    /// Span of the specification call.
    pub span: Span,
}

/// Recognizes calls to `trust2_contract::internal::*` specification helpers.
fn recognize_spec_call(tcx: TyCtxt<'_>, def_id: DefId) -> Option<SpecCallKind> {
    if tcx.crate_name(def_id.krate).as_str() != "trust2_contract" {
        return None;
    }
    let path = tcx.def_path(def_id);
    if path.data.len() != 2 {
        return None;
    }
    let DefPathData::TypeNs(module_sym) = path.data[0].data else {
        return None;
    };
    if module_sym.as_str() != "internal" {
        return None;
    }
    let DefPathData::ValueNs(fn_sym) = path.data[1].data else {
        return None;
    };
    match fn_sym.as_str() {
        "entry" => Some(SpecCallKind::Entry),
        "precondition" => Some(SpecCallKind::Precondition),
        "postcondition" => Some(SpecCallKind::Postcondition),
        "contract_assert" => Some(SpecCallKind::Assert),
        "contract_assume" => Some(SpecCallKind::Assume),
        "forall" => Some(SpecCallKind::ForAll),
        "exists" => Some(SpecCallKind::Exists),
        _ => None,
    }
}

/// Extracts the `SpecCallKind` if `func` is a static call to `trust2_contract::internal::*`.
fn operand_spec_call<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    func: &Operand<'tcx>,
) -> Option<SpecCallKind> {
    let ty::TyKind::FnDef(def_id, _) = *func.ty(&body.local_decls, tcx).kind() else {
        return None;
    };
    recognize_spec_call(tcx, def_id)
}

/// Checks whether `local_def_id` is an implementation of `trust2_contract::internal::TypeInvariant::invariant`.
fn is_type_invariant_impl_fn(tcx: TyCtxt<'_>, local_def_id: LocalDefId) -> bool {
    if tcx.def_kind(local_def_id) != DefKind::AssocFn {
        return false;
    }
    let Some(impl_def_id) = tcx.trait_impl_of_assoc(local_def_id.to_def_id()) else {
        return false;
    };
    let trait_def_id = tcx
        .impl_trait_ref(impl_def_id)
        .instantiate_identity()
        .skip_normalization()
        .def_id;
    if tcx.crate_name(trait_def_id.krate).as_str() != "trust2_contract" {
        return false;
    }
    let path = tcx.def_path(trait_def_id);
    path.data.len() == 2
        && matches!(path.data[0].data, DefPathData::TypeNs(s) if s.as_str() == "internal")
        && matches!(path.data[1].data, DefPathData::TypeNs(s) if s.as_str() == "TypeInvariant")
}

/// Determines whether `local_def_id` is a pure specification body, ensuring any lexical parent's
/// `mir_built` has already run so closure classification is deterministic under `-Zthreads`.
fn classify_spec_body(tcx: TyCtxt<'_>, local_def_id: LocalDefId) -> bool {
    if tcx.def_kind(local_def_id) == DefKind::Closure {
        let parent = tcx.local_parent(local_def_id);
        let _ = tcx.mir_built(parent);
        let mut state = STATE.lock().unwrap();
        if state.spec_bodies.contains(&local_def_id) || state.spec_bodies.contains(&parent) {
            state.spec_bodies.insert(local_def_id);
            return true;
        }
        return false;
    }
    if is_type_invariant_impl_fn(tcx, local_def_id) {
        STATE.lock().unwrap().spec_bodies.insert(local_def_id);
        return true;
    }
    false
}

/// Custom `mir_built` query provider: validates/cleans spec bodies and erases spec captures from
/// host bodies.
fn mir_built_hook<'tcx>(tcx: TyCtxt<'tcx>, def_id: LocalDefId) -> &'tcx Steal<Body<'tcx>> {
    let is_spec = classify_spec_body(tcx, def_id);
    let mut body =
        (rustc_interface::DEFAULT_QUERY_PROVIDERS.queries.mir_built)(tcx, def_id).steal();
    if is_spec {
        process_spec_mir_built(tcx, &mut body);
    } else {
        process_host_mir_built(tcx, def_id, &mut body);
    }
    tcx.alloc_steal_mir(body)
}

/// Custom `mir_promoted` query provider: replaces specification bodies with a trivial self-loop so
/// `mir_borrowck` and drop elaboration never reject non-linear moves inside specifications.
fn mir_promoted_hook<'tcx>(
    tcx: TyCtxt<'tcx>,
    def_id: LocalDefId,
) -> (
    &'tcx Steal<Body<'tcx>>,
    &'tcx Steal<IndexVec<mir::Promoted, Body<'tcx>>>,
) {
    let is_spec = classify_spec_body(tcx, def_id);
    let (body_steal, promoted_steal) = (rustc_interface::DEFAULT_QUERY_PROVIDERS
        .queries
        .mir_promoted)(tcx, def_id);
    if !is_spec {
        return (body_steal, promoted_steal);
    }

    let mut dummy = body_steal.steal();
    dummy.local_decls.truncate(dummy.arg_count + 1);
    dummy.var_debug_info.clear();
    let loop_block = BasicBlock::from_usize(1);
    let mut blocks = IndexVec::new();
    blocks.push(BasicBlockData::new(
        Some(Terminator {
            source_info: SourceInfo::outermost(dummy.span),
            kind: TerminatorKind::Goto { target: loop_block },
        }),
        false,
    ));
    blocks.push(BasicBlockData::new(
        Some(Terminator {
            source_info: SourceInfo::outermost(dummy.span),
            kind: TerminatorKind::Goto { target: loop_block },
        }),
        false,
    ));
    *dummy.basic_blocks_mut() = blocks;
    (tcx.alloc_steal_mir(dummy), promoted_steal)
}

/// Custom `check_liveness` query provider: skips liveness lints on erased specification bodies so
/// by-value captures do not trigger false `unused_assignments` warnings on the substituted self-loop.
fn check_liveness_hook(tcx: TyCtxt<'_>, def_id: LocalDefId) -> DenseBitSet<FieldIdx> {
    if classify_spec_body(tcx, def_id) {
        let num_captures = if tcx.def_kind(def_id) == DefKind::Closure {
            tcx.closure_captures(def_id).len()
        } else {
            0
        };
        return DenseBitSet::new_empty(num_captures);
    }
    (rustc_interface::DEFAULT_QUERY_PROVIDERS
        .queries
        .check_liveness)(tcx, def_id)
}

/// Checks whether `def_id` is an explicit value-destruction function (`core::mem::drop` or
/// `core::ptr::drop_in_place`).
fn is_explicit_drop_fn(tcx: TyCtxt<'_>, def_id: DefId) -> bool {
    if tcx.is_diagnostic_item(sym::mem_drop, def_id) || tcx.is_lang_item(def_id, LangItem::DropGlue)
    {
        return true;
    }
    if tcx.crate_name(def_id.krate) == sym::core {
        let path = tcx.def_path(def_id);
        if matches!(path.data.first(), Some(d) if matches!(d.data, DefPathData::TypeNs(s) if s.as_str() == "ptr"))
            && matches!(path.data.last(), Some(d) if matches!(d.data, DefPathData::ValueNs(s) if s.as_str() == "drop_in_place"))
        {
            return true;
        }
    }
    false
}

/// Checks whether `operand` references `core::mem::drop` or `core::ptr::drop_in_place`.
fn operand_is_explicit_drop<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    operand: &Operand<'tcx>,
) -> bool {
    matches!(
        *operand.ty(&body.local_decls, tcx).kind(),
        ty::TyKind::FnDef(def_id, _) if is_explicit_drop_fn(tcx, def_id)
    )
}

/// Checks whether `rvalue` mentions `core::mem::drop` or `core::ptr::drop_in_place` as a function
/// value (e.g. coercion to `fn(T)` or closure/tuple capture).
fn rvalue_mentions_explicit_drop<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    rvalue: &Rvalue<'tcx>,
) -> bool {
    match rvalue {
        Rvalue::Use(op, ..)
        | Rvalue::Repeat(op, _)
        | Rvalue::Cast(_, op, _)
        | Rvalue::UnaryOp(_, op) => operand_is_explicit_drop(tcx, body, op),
        Rvalue::BinaryOp(_, pair) => {
            operand_is_explicit_drop(tcx, body, &pair.0)
                || operand_is_explicit_drop(tcx, body, &pair.1)
        }
        Rvalue::Aggregate(_, ops) => ops.iter().any(|op| operand_is_explicit_drop(tcx, body, op)),
        _ => false,
    }
}

/// Validates structural purity invariants on a specification MIR body and strips all implicit
/// runtime `Drop` terminators.
fn process_spec_mir_built<'tcx>(tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>) {
    let mut nested_closures = Vec::new();

    for block_data in body.basic_blocks.iter() {
        for stmt in &block_data.statements {
            if let Some((place, rvalue)) = stmt.kind.as_assign() {
                if !place.projection.is_empty() {
                    tcx.dcx().span_err(
                        stmt.source_info.span,
                        "mutating fields or dereferenced references is not allowed in specifications",
                    );
                }
                if rvalue_mentions_explicit_drop(tcx, body, rvalue) {
                    tcx.dcx().span_err(
                        stmt.source_info.span,
                        "explicit `drop` is not allowed in specifications",
                    );
                }
                match rvalue {
                    Rvalue::Ref(_, BorrowKind::Mut { .. }, _) => {
                        tcx.dcx().span_err(
                            stmt.source_info.span,
                            "mutable borrows (`&mut`) are not allowed in specifications; take a shared reference (`&`) instead",
                        );
                    }
                    Rvalue::Aggregate(agg_kind, _) => {
                        if let AggregateKind::Closure(child_def_id, _) = &**agg_kind
                            && let Some(local_child) = child_def_id.as_local()
                        {
                            nested_closures.push(local_child);
                        }
                    }
                    _ => {}
                }
            }
        }
        if let Some(term) = &block_data.terminator
            && let TerminatorKind::Call { func, args, .. } = &term.kind
        {
            if matches!(
                operand_spec_call(tcx, body, func),
                Some(
                    SpecCallKind::Precondition
                        | SpecCallKind::Postcondition
                        | SpecCallKind::Assert
                        | SpecCallKind::Assume
                )
            ) {
                tcx.dcx().span_err(
                    term.source_info.span,
                    "nested contracts or assertions are not allowed inside specifications",
                );
            } else if operand_is_explicit_drop(tcx, body, func) {
                tcx.dcx().span_err(
                    term.source_info.span,
                    "explicit `drop` is not allowed in specifications",
                );
            }
            for arg in args {
                if operand_is_explicit_drop(tcx, body, &arg.node) {
                    tcx.dcx()
                        .span_err(arg.span, "explicit `drop` is not allowed in specifications");
                }
            }
        }
    }

    if let Some(loop_span) = find_non_cleanup_cycle_span(body) {
        tcx.dcx()
            .span_err(loop_span, "loops are not allowed in specifications");
    }

    // Specifications operate on pure mathematical snapshots without runtime lifecycles; strip all
    // implicit `Drop` terminators so scope exits never invoke `Destruct` / `Drop::drop` glue.
    for block_data in body.basic_blocks_mut() {
        if let Some(terminator) = &mut block_data.terminator
            && let TerminatorKind::Drop { target, .. } = terminator.kind
        {
            terminator.kind = TerminatorKind::Goto { target };
        }
    }

    if !nested_closures.is_empty() {
        let mut state = STATE.lock().unwrap();
        state.spec_bodies.extend(nested_closures);
    }
}

/// Detects any cycle among non-cleanup basic blocks in `body`, returning a span for diagnostics.
fn find_non_cleanup_cycle_span(body: &Body<'_>) -> Option<Span> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Color {
        Unvisited,
        Visiting,
        Visited,
    }

    fn dfs(body: &Body<'_>, bb: BasicBlock, colors: &mut [Color]) -> Option<Span> {
        colors[bb.index()] = Color::Visiting;
        let data = &body.basic_blocks[bb];
        if let Some(term) = &data.terminator {
            for succ in term.successors() {
                if body.basic_blocks[succ].is_cleanup {
                    continue;
                }
                match colors[succ.index()] {
                    Color::Visiting => return Some(term.source_info.span),
                    Color::Unvisited => {
                        if let Some(span) = dfs(body, succ, colors) {
                            return Some(span);
                        }
                    }
                    Color::Visited => {}
                }
            }
        }
        colors[bb.index()] = Color::Visited;
        None
    }

    let mut colors = vec![Color::Unvisited; body.basic_blocks.len()];
    if !body.basic_blocks.is_empty() {
        dfs(body, BasicBlock::ZERO, &mut colors)
    } else {
        None
    }
}

/// Scans a host MIR body for `trust2_contract::internal` specification blocks, records their
/// closures and captured places, and erases closure captures from the host MIR.
fn process_host_mir_built<'tcx>(tcx: TyCtxt<'tcx>, host_def_id: LocalDefId, body: &mut Body<'tcx>) {
    let mut sites = Vec::new();
    let mut spec_closures = Vec::new();
    let mut erased_locals = FxHashSet::default();
    let mut next_anchor: u32 = 1;

    let num_blocks = body.basic_blocks.len();
    for bb_idx in 0..num_blocks {
        let bb_entry = BasicBlock::from_usize(bb_idx);
        let Some((entry_dest, bb_spec, entry_span)) = ({
            let Some(term) = &body.basic_blocks[bb_entry].terminator else {
                continue;
            };
            let TerminatorKind::Call {
                func,
                destination,
                target: Some(bb_spec),
                ..
            } = &term.kind
            else {
                continue;
            };
            if operand_spec_call(tcx, body, func) != Some(SpecCallKind::Entry) {
                continue;
            }
            Some((*destination, *bb_spec, term.source_info.span))
        }) else {
            continue;
        };

        let Some((spec_call_kind, spec_args, spec_dest, bb_after, spec_span)) = ({
            let Some(term) = &body.basic_blocks[bb_spec].terminator else {
                continue;
            };
            let TerminatorKind::Call {
                func,
                args,
                destination,
                target: Some(bb_after),
                ..
            } = &term.kind
            else {
                continue;
            };
            let Some(kind) = operand_spec_call(tcx, body, func) else {
                continue;
            };
            Some((
                kind,
                args.clone(),
                *destination,
                *bb_after,
                term.source_info.span,
            ))
        }) else {
            continue;
        };

        if matches!(spec_call_kind, SpecCallKind::ForAll | SpecCallKind::Exists) {
            tcx.dcx().span_err(
                spec_span,
                "quantifiers (`forall!` / `exists!`) can only be used inside specifications",
            );
            for stmt in &body.basic_blocks[bb_spec].statements {
                if let Some((_, Rvalue::Aggregate(agg_kind, _))) = stmt.kind.as_assign()
                    && let AggregateKind::Closure(cid, _) = &**agg_kind
                    && let Some(local_cid) = cid.as_local()
                {
                    spec_closures.push(local_cid);
                }
            }
            continue;
        }

        if spec_call_kind == SpecCallKind::Entry {
            continue;
        }

        let Some(extracted) =
            extract_spec_block_closure(tcx, &body.basic_blocks[bb_spec], &spec_args, spec_span)
        else {
            continue;
        };

        spec_closures.push(extracted.closure_def_id);
        erased_locals.extend(extracted.temp_locals);
        if spec_dest.projection.is_empty() {
            erased_locals.insert(spec_dest.local);
        }

        let mention_stmts: Vec<_> = extracted
            .captures
            .iter()
            .map(|cap| {
                mir::Statement::new(
                    SourceInfo::outermost(cap.span),
                    StatementKind::PlaceMention(Box::new(Place::from(cap.place.local))),
                )
            })
            .collect();

        let host_kind = match spec_call_kind {
            SpecCallKind::Precondition => {
                if entry_dest.projection.is_empty() {
                    erased_locals.insert(entry_dest.local);
                }
                body.basic_blocks_mut()[bb_entry].terminator_mut().kind =
                    TerminatorKind::Goto { target: bb_spec };
                body.basic_blocks_mut()[bb_spec].statements = mention_stmts;
                body.basic_blocks_mut()[bb_spec].terminator_mut().kind =
                    TerminatorKind::Goto { target: bb_after };
                HostSpecKind::Precondition
            }
            SpecCallKind::Postcondition => {
                if entry_dest.projection.is_empty() {
                    erased_locals.insert(entry_dest.local);
                }
                body.basic_blocks_mut()[bb_entry].terminator_mut().kind =
                    TerminatorKind::Goto { target: bb_spec };
                body.basic_blocks_mut()[bb_spec].statements = mention_stmts;
                body.basic_blocks_mut()[bb_spec].terminator_mut().kind =
                    TerminatorKind::Goto { target: bb_after };
                HostSpecKind::Postcondition
            }
            SpecCallKind::Assert | SpecCallKind::Assume => {
                let anchor_id = NonZeroU32::new(next_anchor).unwrap();
                next_anchor += 1;
                let kind = if spec_call_kind == SpecCallKind::Assert {
                    ContractAssertKind::Assert
                } else {
                    ContractAssertKind::Assume
                };
                if let TerminatorKind::Call { args, .. } =
                    &mut body.basic_blocks_mut()[bb_entry].terminator_mut().kind
                    && let Some(first_arg) = args.first_mut()
                {
                    first_arg.node = Operand::Constant(Box::new(ConstOperand {
                        span: entry_span,
                        user_ty: None,
                        const_: Const::Val(
                            ConstValue::Scalar(Scalar::from_u32(anchor_id.get())),
                            tcx.types.u32,
                        ),
                    }));
                }
                let entry_local = entry_dest.local;
                let spec_stmts = &mut body.basic_blocks_mut()[bb_spec].statements;
                spec_stmts.retain(
                    |stmt| matches!(stmt.kind, StatementKind::StorageDead(l) if l == entry_local),
                );
                spec_stmts.extend(mention_stmts);
                body.basic_blocks_mut()[bb_spec].terminator_mut().kind =
                    TerminatorKind::Goto { target: bb_after };
                HostSpecKind::InlineAssert { kind, anchor_id }
            }
            SpecCallKind::Entry | SpecCallKind::ForAll | SpecCallKind::Exists => unreachable!(),
        };

        sites.push(HostSpecSite {
            kind: host_kind,
            closure_def_id: extracted.closure_def_id,
            captures: extracted.captures.into_boxed_slice(),
            span: spec_span,
        });
    }

    if !erased_locals.is_empty() {
        for &local in &erased_locals {
            body.local_decls[local].ty = tcx.types.unit;
            body.local_decls[local].user_ty = None;
        }
        for block_data in body.basic_blocks_mut() {
            block_data.statements.retain(|stmt| match &stmt.kind {
                StatementKind::StorageLive(l) | StatementKind::StorageDead(l) => {
                    !erased_locals.contains(l)
                }
                StatementKind::FakeRead(pair) => !erased_locals.contains(&pair.1.local),
                StatementKind::PlaceMention(p) => !erased_locals.contains(&p.local),
                StatementKind::Assign(pair) => !erased_locals.contains(&pair.0.local),
                _ => true,
            });
        }
    }

    if !spec_closures.is_empty() || !sites.is_empty() {
        let mut state = STATE.lock().unwrap();
        state.spec_bodies.extend(spec_closures);
        if state.record_host_sites && !sites.is_empty() {
            state.host_sites.insert(host_def_id, sites);
        }
    }
}

struct ExtractedSpecClosure {
    closure_def_id: LocalDefId,
    captures: Vec<SavedCapture>,
    temp_locals: Vec<Local>,
}

/// Substitutes capture-preparation temporaries (`CopyForDeref`, `Use(Copy/Move)`, and `*(&place)`)
/// recorded earlier in `bb_spec` so the resulting `mir::Place` roots at an un-erased host local.
fn resolve_capture_place<'tcx>(
    tcx: TyCtxt<'tcx>,
    place: Place<'tcx>,
    place_aliases: &FxHashMap<Local, (Place<'tcx>, Span)>,
    ref_assigns: &FxHashMap<Local, (BorrowKind, Place<'tcx>, Span)>,
) -> Place<'tcx> {
    let remap_elem = |elem: PlaceElem<'tcx>| match elem {
        ProjectionElem::Index(idx_local) => match place_aliases.get(&idx_local) {
            Some(&(aliased, _)) if aliased.projection.is_empty() => {
                ProjectionElem::Index(aliased.local)
            }
            _ => elem,
        },
        _ => elem,
    };

    if let Some(&(base_place, _)) = place_aliases.get(&place.local) {
        let mut elems = Vec::with_capacity(base_place.projection.len() + place.projection.len());
        elems.extend(base_place.projection.iter());
        elems.extend(place.projection.iter().map(remap_elem));
        return Place {
            local: base_place.local,
            projection: tcx.mk_place_elems(&elems),
        };
    }

    if let Some(&(_, target_place, _)) = ref_assigns.get(&place.local)
        && place.projection.first() == Some(&ProjectionElem::Deref)
    {
        let tail = &place.projection[1..];
        let mut elems = Vec::with_capacity(target_place.projection.len() + tail.len());
        elems.extend(target_place.projection.iter());
        elems.extend(tail.iter().copied().map(remap_elem));
        return Place {
            local: target_place.local,
            projection: tcx.mk_place_elems(&elems),
        };
    }

    if place
        .projection
        .iter()
        .any(|e| matches!(e, ProjectionElem::Index(l) if place_aliases.contains_key(&l)))
    {
        let elems: Vec<_> = place.projection.iter().map(remap_elem).collect();
        return Place {
            local: place.local,
            projection: tcx.mk_place_elems(&elems),
        };
    }

    place
}

/// Checks whether `place` still references any erased capture-preparation temporary in `temp_set`.
fn place_mentions_temp(place: Place<'_>, temp_set: &FxHashSet<Local>) -> bool {
    temp_set.contains(&place.local)
        || place
            .projection
            .iter()
            .any(|elem| matches!(elem, ProjectionElem::Index(l) if temp_set.contains(&l)))
}

/// Extracts the closure `LocalDefId`, captured places, and temporary locals from `bb_spec`.
fn extract_spec_block_closure<'tcx>(
    tcx: TyCtxt<'tcx>,
    bb_spec: &BasicBlockData<'tcx>,
    spec_args: &[Spanned<Operand<'tcx>>],
    spec_span: Span,
) -> Option<ExtractedSpecClosure> {
    let [closure_arg] = spec_args else {
        tcx.dcx().span_err(
            spec_span,
            "expected a single closure argument in specification",
        );
        return None;
    };
    let Operand::Move(closure_place) = closure_arg.node else {
        tcx.dcx()
            .span_err(spec_span, "expected a closure literal in specification");
        return None;
    };
    if !closure_place.projection.is_empty() {
        tcx.dcx()
            .span_err(spec_span, "expected a closure literal in specification");
        return None;
    }

    let mut place_aliases: FxHashMap<Local, (Place<'tcx>, Span)> = FxHashMap::default();
    let mut ref_assigns: FxHashMap<Local, (BorrowKind, Place<'tcx>, Span)> = FxHashMap::default();
    let mut temp_set: FxHashSet<Local> = FxHashSet::default();
    temp_set.insert(closure_place.local);
    let mut temp_locals = vec![closure_place.local];
    let mut closure_aggregate: Option<(LocalDefId, Vec<Operand<'tcx>>, Span)> = None;

    for stmt in &bb_spec.statements {
        let Some((lhs, rhs)) = stmt.kind.as_assign() else {
            continue;
        };
        if !lhs.projection.is_empty() {
            tcx.dcx().span_err(
                stmt.source_info.span,
                "expected an inline closure literal in specification",
            );
            return None;
        }
        match rhs {
            Rvalue::CopyForDeref(src) | Rvalue::Use(Operand::Copy(src) | Operand::Move(src), _) => {
                let resolved = resolve_capture_place(tcx, *src, &place_aliases, &ref_assigns);
                place_aliases.insert(lhs.local, (resolved, stmt.source_info.span));
                if temp_set.insert(lhs.local) {
                    temp_locals.push(lhs.local);
                }
            }
            Rvalue::Ref(_, borrow_kind, place) => {
                let resolved = resolve_capture_place(tcx, *place, &place_aliases, &ref_assigns);
                ref_assigns.insert(lhs.local, (*borrow_kind, resolved, stmt.source_info.span));
                if temp_set.insert(lhs.local) {
                    temp_locals.push(lhs.local);
                }
            }
            Rvalue::Aggregate(agg_kind, operands) if lhs.local == closure_place.local => {
                if let AggregateKind::Closure(def_id, _) = &**agg_kind
                    && let Some(local_def_id) = def_id.as_local()
                {
                    closure_aggregate = Some((
                        local_def_id,
                        operands.iter().cloned().collect(),
                        stmt.source_info.span,
                    ));
                }
            }
            _ => {
                tcx.dcx().span_err(
                    stmt.source_info.span,
                    "expected an inline closure literal in specification",
                );
                return None;
            }
        }
    }

    let Some((closure_def_id, operands, agg_span)) = closure_aggregate else {
        tcx.dcx().span_err(
            spec_span,
            "expected an inline closure literal in specification",
        );
        return None;
    };

    let mut captures = Vec::with_capacity(operands.len());
    for operand in operands {
        match operand {
            Operand::Move(place) | Operand::Copy(place)
                if place.projection.is_empty() && ref_assigns.contains_key(&place.local) =>
            {
                let (borrow_kind, target_place, ref_span) = ref_assigns[&place.local];
                if matches!(borrow_kind, BorrowKind::Mut { .. }) {
                    tcx.dcx().span_err(
                        ref_span,
                        "specifications cannot mutably borrow or mutate program variables",
                    );
                    return None;
                }
                if place_mentions_temp(target_place, &temp_set) {
                    tcx.dcx().span_err(
                        ref_span,
                        "expected an inline closure literal in specification",
                    );
                    return None;
                }
                let Some(saved_place) = SavedPlace::from_mir(target_place) else {
                    tcx.dcx().span_err(
                        ref_span,
                        "unsupported place projection captured in specification",
                    );
                    return None;
                };
                captures.push(SavedCapture {
                    mode: CaptureMode::SharedRef,
                    place: saved_place,
                    span: ref_span,
                });
            }
            Operand::Move(place) | Operand::Copy(place) => {
                let cap_span = if place.projection.is_empty() {
                    place_aliases
                        .get(&place.local)
                        .map_or(agg_span, |&(_, span)| span)
                } else {
                    agg_span
                };
                let resolved = resolve_capture_place(tcx, place, &place_aliases, &ref_assigns);
                if place_mentions_temp(resolved, &temp_set) {
                    tcx.dcx().span_err(
                        cap_span,
                        "expected an inline closure literal in specification",
                    );
                    return None;
                }
                let Some(saved_place) = SavedPlace::from_mir(resolved) else {
                    tcx.dcx().span_err(
                        cap_span,
                        "unsupported place projection captured in specification",
                    );
                    return None;
                };
                captures.push(SavedCapture {
                    mode: CaptureMode::ByValue,
                    place: saved_place,
                    span: cap_span,
                });
            }
            Operand::Constant(_) | Operand::RuntimeChecks(_) => {
                tcx.dcx().span_err(
                    agg_span,
                    "unexpected constant upvar in specification closure",
                );
                return None;
            }
        }
    }

    Some(ExtractedSpecClosure {
        closure_def_id,
        captures,
        temp_locals,
    })
}

/// Forces `mir_built` across the crate, snapshots pre-borrowck MIR for all specification bodies,
/// and checks host borrow/initialization legality at every specification anchor.
///
/// Returns `Err(())` if `rustc` or specification legality checks reported any errors.
pub(crate) fn prepare_crate_specs<'tcx>(
    tcx: TyCtxt<'tcx>,
    options: &CliOpts,
) -> Result<PreparedSpecMir<'tcx>, ()> {
    for def_id in tcx.hir_body_owners() {
        let def_kind = tcx.def_kind(def_id);
        if !matches!(def_kind, DefKind::AnonConst | DefKind::InlineConst) {
            let _ = tcx.mir_built(def_id);
        }
    }

    if tcx.dcx().has_errors().is_some() {
        return Err(());
    }

    let (spec_ids, host_sites): (Vec<LocalDefId>, FxHashMap<LocalDefId, Vec<HostSpecSite>>) = {
        let mut state = STATE.lock().unwrap();
        (
            state.spec_bodies.iter().copied().collect(),
            std::mem::take(&mut state.host_sites),
        )
    };

    let mut spec_mir_bodies = FxHashMap::default();
    for spec_id in spec_ids {
        let mir_steal = tcx.mir_built(spec_id);
        assert!(
            !mir_steal.is_stolen(),
            "specification `mir_built` for {spec_id:?} was unexpectedly stolen before snapshot"
        );
        spec_mir_bodies.insert(spec_id, mir_steal.borrow().clone());
    }

    let mut borrowck_bodies: FxHashMap<LocalDefId, BodyWithBorrowckFacts<'tcx>> =
        FxHashMap::default();
    if !options.skip_borrowck {
        for (&host_def_id, sites) in &host_sites {
            let root = tcx
                .typeck_root_def_id(host_def_id.to_def_id())
                .expect_local();
            if !borrowck_bodies.contains_key(&host_def_id) {
                borrowck_bodies.extend(get_bodies_with_borrowck_facts(
                    tcx,
                    root,
                    ConsumerOptions::PoloniusInputFacts,
                ));
            }
            // Also run standard `mir_borrowck` so any ordinary borrowck diagnostics in `root` are
            // emitted before we inspect `facts`.
            let _ = tcx.mir_borrowck(root);
            if tcx.dcx().has_errors().is_some() {
                return Err(());
            }
            if let Some(facts) = borrowck_bodies.get(&host_def_id) {
                validate_host_sites_at_anchors(tcx, facts, sites);
            }
        }
        if tcx.dcx().has_errors().is_some() {
            return Err(());
        }
    }

    Ok(PreparedSpecMir {
        spec_mir_bodies,
        borrowck_bodies,
        host_sites,
    })
}

/// Checks uninit and active `&mut` borrow conflicts for every captured values in `facts.body`.
fn validate_host_sites_at_anchors<'tcx>(
    tcx: TyCtxt<'tcx>,
    facts: &BodyWithBorrowckFacts<'tcx>,
    sites: &[HostSpecSite],
) {
    let body = &facts.body;
    let anchor_locations = collect_anchor_locations(tcx, body);

    let move_data = MoveData::gather_moves(body, tcx, |_| true);
    let mut uninit_cursor = MaybeUninitializedPlaces::new(tcx, body, &move_data)
        .mark_inactive_variants_as_uninit()
        .iterate_to_fixpoint(tcx, body, None)
        .into_results_cursor(body);
    let mut borrows_cursor = Borrows::new(
        tcx,
        body,
        &facts.region_inference_context,
        &facts.borrow_set,
    )
    .iterate_to_fixpoint(tcx, body, None)
    .into_results_cursor(body);

    for site in sites {
        if site.captures.is_empty() {
            continue;
        }
        let location = match site.kind {
            HostSpecKind::Precondition | HostSpecKind::Postcondition => Location::START,
            HostSpecKind::InlineAssert { anchor_id, .. } => {
                let Some(&loc) = anchor_locations.get(&anchor_id.get()) else {
                    // Anchor block is statically unreachable in promoted MIR.
                    continue;
                };
                loc
            }
        };

        uninit_cursor.seek_before_primary_effect(location);
        borrows_cursor.seek_before_primary_effect(location);

        for capture in &site.captures {
            let mir_place = capture.place.to_mir_place(tcx, body);

            // 1. Check that the captured place (and all of its sub-places) is initialized and has
            //    not been moved out of.
            let is_uninit = match move_data.rev_lookup.find(mir_place.as_ref()) {
                LookupResult::Exact(mp) => {
                    let mut any_uninit = false;
                    on_all_children_bits(&move_data, mp, |child_mp| {
                        if uninit_cursor.get().contains(child_mp) {
                            any_uninit = true;
                        }
                    });
                    any_uninit
                }
                LookupResult::Parent(Some(mp)) => uninit_cursor.get().contains(mp),
                LookupResult::Parent(None) => false,
            };
            if is_uninit {
                tcx.dcx().span_err(
                    capture.span,
                    "cannot read moved or uninitialized value in specification",
                );
                continue;
            }

            // 2. Check that no active mutable borrow overlaps the captured place.
            for borrow_idx in borrows_cursor.get().iter() {
                let loan = &facts.borrow_set[borrow_idx];
                if !matches!(loan.kind(), BorrowKind::Mut { .. }) {
                    continue;
                }
                if places_conflict(
                    tcx,
                    body,
                    loan.borrowed_place(),
                    mir_place,
                    PlaceConflictBias::Overlap,
                ) {
                    let loan_loc = loan.reserve_location();
                    let loan_span = body
                        .basic_blocks
                        .get(loan_loc.block)
                        .and_then(|bb| {
                            bb.statements
                                .get(loan_loc.statement_index)
                                .map(|s| s.source_info.span)
                                .or_else(|| bb.terminator.as_ref().map(|t| t.source_info.span))
                        })
                        .unwrap_or(DUMMY_SP);
                    let mut diag = tcx.dcx().struct_span_err(
                        capture.span,
                        "cannot read mutably borrowed value in specification",
                    );
                    if !loan_span.is_dummy() {
                        diag.span_label(loan_span, "mutable borrow is active here");
                    }
                    diag.help(
                        "read the current value through the active mutable reference instead",
                    );
                    diag.emit();
                    break;
                }
            }
        }
    }
}

/// Maps 1-based `anchor_id`s from `internal::entry(anchor_id)` calls to their `mir::Location` in `body`.
fn collect_anchor_locations<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
) -> FxHashMap<u32, Location> {
    let mut map = FxHashMap::default();
    for (bb, data) in body.basic_blocks.iter_enumerated() {
        let Some(term) = &data.terminator else {
            continue;
        };
        let TerminatorKind::Call { func, args, .. } = &term.kind else {
            continue;
        };
        if operand_spec_call(tcx, body, func) != Some(SpecCallKind::Entry) {
            continue;
        }
        if let Some(arg) = args.first()
            && let Operand::Constant(c) = &arg.node
            && let Some(val) = c.const_.try_to_bits(Size::from_bytes(4))
        {
            let anchor_id = val as u32;
            if anchor_id != 0 {
                map.insert(
                    anchor_id,
                    Location {
                        block: bb,
                        statement_index: data.statements.len(),
                    },
                );
            }
        }
    }
    map
}
