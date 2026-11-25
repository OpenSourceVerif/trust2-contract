//! Owned projection trees, indexed by the declaration order of Charon fields.
use crate::{
    AnalysisError, FieldInfo, LocalInfo, Place,
    input::{invalid, scalar, unsupported},
};
use charon_lib::ast::{self as a, *};
use std::collections::{BTreeMap, BTreeSet};

impl LocalInfo {
    pub fn children(&self, base: Place) -> impl Iterator<Item = Place> + '_ {
        self.fields
            .iter()
            .enumerate()
            .filter_map(move |(projection, f)| {
                (f.parent == base).then_some(Place::Field {
                    local: self.id,
                    projection,
                })
            })
    }
    pub fn field(&self, place: Place) -> Option<&FieldInfo> {
        if let Place::Field { local, projection } = place
            && local == self.id
        {
            self.fields.get(projection)
        } else {
            None
        }
    }
}

pub(crate) struct Places {
    pub locals: Vec<LocalInfo>,
    types: BTreeMap<Place, Ty>,
}
impl Places {
    pub fn new(locals: &Locals, krate: Option<&TranslatedCrate>) -> Result<Self, AnalysisError> {
        let mut result = Self {
            locals: vec![],
            types: BTreeMap::new(),
        };
        for (id, local) in locals.locals.iter_enumerated() {
            let mut info = LocalInfo {
                id,
                name: local.name.clone(),
                fields: vec![],
            };
            result.add(
                &mut info,
                Place::Local(id),
                &local.ty,
                krate,
                &mut BTreeSet::new(),
            )?;
            result.locals.push(info);
        }
        Ok(result)
    }

    fn add(
        &mut self,
        info: &mut LocalInfo,
        place: Place,
        ty: &Ty,
        krate: Option<&TranslatedCrate>,
        ancestors: &mut BTreeSet<Ty>,
    ) -> Result<(), AnalysisError> {
        self.types.insert(place, ty.clone());
        if scalar(ty) {
            return Ok(());
        }
        if let TyKind::Ref(_, inner, _) = ty.kind() {
            if matches!(place, Place::Local(_)) && scalar(inner) {
                return Ok(());
            }
            return Err(unsupported(
                "reference-bearing aggregate or reference to non-scalar",
            ));
        }
        let Some(tref) = ty.as_adt() else {
            return Err(unsupported(format!("unsupported place type {ty:?}")));
        };
        let fields: Vec<(Option<String>, Ty)> = match tref.id {
            TypeId::Tuple => tref
                .generics
                .types
                .iter()
                .cloned()
                .map(|t| (None, t))
                .collect(),
            TypeId::Adt(id) => {
                let decl = krate.and_then(|k| k.type_decls.get(id)).ok_or_else(|| {
                    unsupported("the type declaration is required to expand a struct")
                })?;
                if !tref.generics.matches(&decl.generics) {
                    return Err(invalid(
                        "struct arguments do not match its type declaration",
                    ));
                }
                let TypeDeclKind::Struct(fs) = &decl.kind else {
                    return Err(unsupported(
                        "enum, union or opaque type requires its own projection rules",
                    ));
                };
                fs.iter()
                    .map(|f| (f.name.clone(), f.ty.clone().substitute(&tref.generics)))
                    .collect()
            }
            _ => return Err(unsupported(format!("unsupported aggregate {ty:?}"))),
        };
        if !ancestors.insert(ty.clone()) {
            return Err(invalid("recursive by-value field layout"));
        }
        for (index, (name, ty)) in fields.into_iter().enumerate() {
            let sub = Place::Field {
                local: info.id,
                projection: info.fields.len(),
            };
            info.fields.push(FieldInfo {
                parent: place,
                index: FieldId::from(index),
                name,
            });
            self.add(info, sub, &ty, krate, ancestors)?;
        }
        ancestors.remove(ty);
        Ok(())
    }

    pub fn ty(&self, place: Place) -> &Ty {
        &self.types[&place]
    }

    pub fn lower(&self, p: &a::Place) -> Result<Place, AnalysisError> {
        let place = match &p.kind {
            PlaceKind::Local(id) => Place::Local(*id),
            PlaceKind::Projection(base, ProjectionElem::Deref) => {
                let Place::Local(id) = self.lower(base)? else {
                    return Err(unsupported("nested dereference"));
                };
                if !matches!(base.ty.kind(),TyKind::Ref(_,inner,_) if inner==&p.ty) {
                    return Err(invalid("dereference type mismatch"));
                }
                return Ok(Place::Deref(id));
            }
            PlaceKind::Projection(base, ProjectionElem::Field(kind, index)) => {
                let parent = self.lower(base)?;
                match (kind, base.ty.as_adt()) {
                    (FieldProjKind::Tuple(n), Some(t))
                        if t.id == TypeId::Tuple && t.generics.types.len() == *n => {}
                    (FieldProjKind::Adt(id, None), Some(t)) if t.id == TypeId::Adt(*id) => {}
                    _ => return Err(invalid("field projection does not match its type")),
                }
                let local = parent.local();
                let projection = self.locals[local.index()]
                    .fields
                    .iter()
                    .position(|f| f.parent == parent && f.index == *index)
                    .ok_or_else(|| invalid("unknown field on place"))?;
                Place::Field { local, projection }
            }
            _ => return Err(unsupported("index, raw-pointer or global projection")),
        };
        if self.types.get(&place) != Some(&p.ty) {
            return Err(invalid("place type differs from its declaration"));
        }
        Ok(place)
    }
}
