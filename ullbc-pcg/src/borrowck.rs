//! Validated ULLBC-level query interface. MIR identities never enter the engine.
use crate::{
    input::{Operation, Program, Rhs},
    *,
};
use charon_lib::{ast::*, ullbc_ast as u};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) struct BorrowContext<'a> {
    points: BTreeMap<Location, &'a BorrowckPoint>,
    loans: BTreeMap<Location, &'a BorrowckLoan>,
    regions: BTreeMap<LocalId, usize>,
    outlives: BTreeSet<(usize, usize)>,
}
fn invalid(message: impl Into<String>) -> AnalysisError {
    AnalysisError::new(ErrorKind::InvalidInput, message)
}
impl From<BorrowckLocation> for Location {
    fn from(value: BorrowckLocation) -> Self {
        Self {
            block: value.block,
            statement_index: value.statement_index,
        }
    }
}
impl<'a> BorrowContext<'a> {
    pub fn new(
        body: &u::ExprBody,
        facts: &'a BodyBorrowckInfo,
        program: &Program,
    ) -> Result<Self, AnalysisError> {
        if facts.body_fingerprint != borrowck_body_fingerprint(body) {
            return Err(invalid(
                "borrowck facts do not match this ULLBC body; re-extract with --export-borrowck",
            ));
        }
        if let Some(reason) = &facts.unsupported {
            return Err(input::unsupported(reason));
        }
        let mut points = BTreeMap::new();
        for point in &facts.points {
            let location = Location::from(point.location);
            if points.insert(location, point).is_some() {
                return Err(invalid("duplicate borrowck point").at(location));
            }
            let block = body
                .body
                .get(location.block)
                .ok_or_else(|| invalid("borrowck point names missing block"))?;
            if location.statement_index > block.statements.len() {
                return Err(invalid("borrowck point outside block").at(location));
            }
            for set in [&point.before, &point.after] {
                if set.iter().any(|id| *id >= facts.loans.len())
                    || set.iter().collect::<BTreeSet<_>>().len() != set.len()
                {
                    return Err(invalid("unknown/duplicate loan in borrowck point").at(location));
                }
            }
        }
        let expected_points: usize = body.body.iter().map(|b| b.statements.len() + 1).sum();
        if points.len() != expected_points {
            return Err(invalid("incomplete borrowck point coverage"));
        }
        let mut loans = BTreeMap::new();
        for (index, loan) in facts.loans.iter().enumerate() {
            let reserve = Location::from(loan.reserve);
            if loan.id != index || loans.insert(reserve, loan).is_some() {
                return Err(invalid("invalid loan identities/reservation anchors"));
            }
            let Some(op) = program
                .blocks
                .get(&reserve.block)
                .and_then(|b| b.operations.get(reserve.statement_index))
            else {
                return Err(invalid("loan reservation outside analyzed CFG"));
            };
            if !matches!(op, Operation::Assign(crate::Place::Local(target), Rhs::Borrow(kind, owner))
                if *target == loan.assigned && *owner == loan.borrowed && (*kind == crate::BorrowKind::Mutable) == loan.is_mutable)
            {
                return Err(invalid("loan does not match ULLBC borrow assignment").at(reserve));
            }
        }
        for (&block, data) in &program.blocks {
            for (statement_index, op) in data.operations.iter().enumerate() {
                let location = Location {
                    block,
                    statement_index,
                };
                if matches!(op, Operation::Assign(_, Rhs::Borrow(..)))
                    && !loans.contains_key(&location)
                {
                    return Err(invalid("ULLBC borrow has no compiler loan").at(location));
                }
            }
        }
        let regions: BTreeMap<_, _> = facts
            .reference_regions
            .iter()
            .map(|r| (r.local, r.region))
            .collect();
        if regions.len() != facts.reference_regions.len()
            || regions.keys().copied().collect::<BTreeSet<_>>() != program.references
        {
            return Err(invalid(
                "reference-region bindings do not cover the ULLBC reference locals",
            ));
        }
        let known: BTreeSet<_> = regions
            .values()
            .copied()
            .chain(facts.loans.iter().map(|l| l.region))
            .collect();
        let outlives: BTreeSet<_> = facts
            .outlives
            .iter()
            .map(|p| (p.longer, p.shorter))
            .collect();
        if outlives.len() != facts.outlives.len()
            || outlives
                .iter()
                .any(|(a, b)| !known.contains(a) || !known.contains(b))
            || known.iter().any(|r| !outlives.contains(&(*r, *r)))
        {
            return Err(invalid("invalid/incomplete outlives relation"));
        }
        Ok(Self {
            points,
            loans,
            regions,
            outlives,
        })
    }
    pub fn point(&self, location: Location) -> &BorrowckPoint {
        self.points[&location]
    }
    pub fn loan(&self, creation: Location) -> &BorrowckLoan {
        self.loans[&creation]
    }
    /// Only before-primary state is used during PCG transfer. After-primary facts
    /// must not be misinterpreted as the intermediate PostOperands/PreMain phase.
    pub fn in_scope(&self, creation: Location, at: Location) -> bool {
        self.point(at).before.contains(&self.loan(creation).id)
    }
    pub fn outlives(&self, from: LocalId, to: LocalId) -> bool {
        self.outlives
            .contains(&(self.regions[&from], self.regions[&to]))
    }
}
