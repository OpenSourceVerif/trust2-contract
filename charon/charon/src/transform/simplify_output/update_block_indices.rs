//! Update the block indices to make sure they are consecutive

use std::mem;

use crate::ids::*;
use crate::transform::TransformCtx;
use crate::ullbc_ast::*;

use crate::transform::ctx::UllbcPass;

pub struct Transform;
impl UllbcPass for Transform {
    fn transform_body(&self, ctx: &mut TransformCtx, b: &mut ExprBody) {
        let mut facts = crate::transform::borrowck::take(ctx, b);
        // Push each block into a new vector to make it consecutive and return the map from old to
        // new ids.
        let id_map: IndexVec<BlockId, BlockId> =
            mem::take(&mut b.body).map(|block| b.body.push(block));

        // Update the ids.
        b.body
            .dyn_visit_in_body_mut(|id: &mut BlockId| *id = id_map[*id]);
        if let Some(info) = facts.as_mut() {
            crate::transform::borrowck::remap(info, |mut p| {
                p.block = id_map.get(p.block).copied()?;
                Some(p)
            });
        }
        crate::transform::borrowck::put(ctx, b, facts);
    }
}
