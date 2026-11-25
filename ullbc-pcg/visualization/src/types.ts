// The version 4 contract (older version 3 snapshots also open) emitted by ullbc-pcg/src/export.rs.
export type SourcePos = { line: number; column: number };
export type FunctionSlug = string;
export type FunctionsMetadata = Record<string, {
  name: string; signature: string; debug_signature: string;
  source: string; start: SourcePos;
}>;
export type Place = { Local: number } | { Deref: number } | { Field: { local: number; projection: number } };
type CfgEdge = { from: number; to: number };
type GraphEdge = { kind: "Borrow" | "Deref" | "BorrowFlow" | "InitialBorrowFlow" | "CallAbstraction" };
export type Graph = {
  expanded?: Place[];
  initialized_fields?: Place[];
  initialized_referents?: number[];
  edge_conditions?: { edge: GraphEdge; alternatives: CfgEdge[][] }[];
  capabilities: { place: Place; capability: "Read" | "Write" | "Exclusive" | "ShallowExclusive" }[];
  edges: GraphEdge[];
};
export type Phase = "pre_operands" | "post_operands" | "pre_main" | "post_main";
export type Step = {
  kind: "initial" | "phase" | "action";
  phase: Phase | null;
  action_index: number | null;
  label: string;
  dot: string;
  graph: Graph;
};
export type Location = { block: number; statement_index: number };
export type BorrowckQuery =
  | { LoanInScope: { loan: number; location: Location; result: boolean } }
  | { Outlives: { from: number; to: number; result: boolean } };
export type Point = {
  location: Location;
  statement: string;
  span: { low: SourcePos; high: SourcePos } | null;
  steps: Step[];
  action_labels: string[];
  live_references_after: number[];
  loans_before: number[];
  loans_after: number[];
  phases: { phase: Phase; borrowck_queries: BorrowckQuery[] }[];
};
export type Analysis = {
  schema_version: number; name: string; signature: string; source: string;
  source_start_line: number;
  locals: { id: number; name: string | null; fields?: { parent: Place; index: number; name: string | null }[] }[];
  points: Point[];
  blocks: { block: number; successors: number[]; entry: Graph; exit: Graph }[];
  analysis_stage?: "before-control-flow-reconstruction" | "final-ullbc";
  borrowck_source: string;
};
