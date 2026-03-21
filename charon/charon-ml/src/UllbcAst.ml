open Types
open Values
open Expressions
open Meta
open Identifiers
include GAst
include Generated_UllbcAst

type expr_body = (blocks, fun_specs) gexpr_body [@@deriving show]
type fun_body = expr_body [@@deriving show]
type fun_decl = (blocks, fun_specs) gfun_decl [@@deriving show]

(** ULLBC crate *)
type crate = (blocks, fun_specs) gcrate [@@deriving show]
