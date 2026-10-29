//! Capture facts on translated ULLBC; normal passes adapt them to final ULLBC.
use charon_lib::{ast::*, ullbc_ast as u};
use rustc_borrowck::consumers::{BodyWithBorrowckFacts, Borrows, TwoPhaseActivation};
use rustc_middle::{mir, ty};
use rustc_mir_dataflow::Analysis;
use std::collections::{BTreeSet, HashMap};

/// Translate the exact borrowck body, erasing only its types' region names.
/// The original body stays alive alongside its matching inference context.
pub fn erase_regions<'tcx>(tcx: ty::TyCtxt<'tcx>, mut body: mir::Body<'tcx>) -> mir::Body<'tcx> {
    use mir::visit::{MutVisitor, TyContext};
    struct Erase<'tcx>(ty::TyCtxt<'tcx>);
    impl<'tcx> MutVisitor<'tcx> for Erase<'tcx> {
        fn tcx(&self) -> ty::TyCtxt<'tcx> {
            self.0
        }
        fn visit_ty(&mut self, value: &mut ty::Ty<'tcx>, _: TyContext) {
            *value = self.0.erase_and_anonymize_regions(*value);
        }
        fn visit_region(&mut self, value: &mut ty::Region<'tcx>, _: mir::Location) {
            *value = self.0.lifetimes.re_erased;
        }
    }
    Erase(tcx).visit_body(&mut body);
    body
}

pub fn adapt<'tcx>(
    tcx: ty::TyCtxt<'tcx>,
    facts: &BodyWithBorrowckFacts<'tcx>,
    function: FunDeclId,
    locals: &HashMap<usize, LocalId>,
    anchors: &[(mir::Location, BorrowckLocation, usize)],
    body: &mut u::ExprBody,
) -> BodyBorrowckInfo {
    let mut info = BodyBorrowckInfo {
        function_id: function,
        unsupported: None,
        body_fingerprint: String::new(),
        loans: Vec::new(),
        points: Vec::new(),
        reference_regions: Vec::new(),
        outlives: Vec::new(),
    };
    let result = (|| -> Result<(), String> {
        let mut map = HashMap::new();
        for &(mir, ullbc, count) in anchors {
            if count > 1 {
                return Err(
                    "borrowck adapter: MIR operation expands into multiple ULLBC operations".into(),
                );
            }
            if count == 1 {
                map.insert(mir, ullbc);
            }
        }
        let mut regions = BTreeSet::new();
        for (local, decl) in facts.body.local_decls.iter_enumerated() {
            if let ty::Ref(region, _, _) = decl.ty.kind() {
                let ty::ReVar(vid) = region.kind() else {
                    return Err("borrowck adapter: non-inference reference region".into());
                };
                regions.insert(vid);
                info.reference_regions.push(BorrowckReferenceRegion {
                    local: locals[&local.index()],
                    region: vid.index(),
                });
            }
        }
        for (id, loan) in facts.borrow_set.location_map().values().enumerate() {
            if !loan.borrowed_place().projection.is_empty()
                || !loan.assigned_place().projection.is_empty()
            {
                return Err(
                    "borrowck adapter: projected loans/reborrows are not supported yet".into(),
                );
            }
            if loan.activation_location() != TwoPhaseActivation::NotTwoPhase {
                return Err("borrowck adapter: two-phase loans are not supported yet".into());
            }
            let mutable = match loan.kind() {
                mir::BorrowKind::Shared => false,
                mir::BorrowKind::Mut {
                    kind: mir::MutBorrowKind::Default,
                } => true,
                _ => return Err("borrowck adapter: unsupported borrow kind".into()),
            };
            let reserve = *map
                .get(&loan.reserve_location())
                .ok_or("borrowck adapter: loan reservation has no unique ULLBC anchor")?;
            regions.insert(loan.region());
            info.loans.push(BorrowckLoan {
                id,
                reserve,
                borrowed: locals[&loan.borrowed_place().local.index()],
                assigned: locals[&loan.assigned_place().local.index()],
                is_mutable: mutable,
                region: loan.region().index(),
            });
        }
        let mut cursor = Borrows::new(
            tcx,
            &facts.body,
            &facts.region_inference_context,
            &facts.borrow_set,
        )
        .iterate_to_fixpoint(tcx, &facts.body, None)
        .into_results_cursor(&facts.body);
        for &(mir, location, count) in anchors {
            if count == 0 {
                continue;
            }
            cursor.seek_before_primary_effect(mir);
            let before = cursor.get().iter().map(|id| id.index()).collect();
            cursor.seek_after_primary_effect(mir);
            let after = cursor.get().iter().map(|id| id.index()).collect();
            info.points.push(BorrowckPoint {
                location,
                synthetic: false,
                before,
                after,
            });
        }
        for &longer in &regions {
            for &shorter in &regions {
                if facts
                    .region_inference_context
                    .eval_outlives(longer, shorter)
                {
                    info.outlives.push(BorrowckOutlives {
                        longer: longer.index(),
                        shorter: shorter.index(),
                    });
                }
            }
        }
        Ok(())
    })();
    if let Err(reason) = result {
        info.unsupported = Some(reason);
    }
    info.points.sort_by_key(|p| p.location);
    info.body_fingerprint = borrowck_body_fingerprint(body);
    info
}
