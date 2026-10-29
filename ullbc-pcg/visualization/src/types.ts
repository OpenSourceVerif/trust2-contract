// The version 3 contract emitted by ullbc-pcg/src/export.rs.
export type SourcePos = { line: number; column: number };
export type FunctionSlug = string;
export type FunctionsMetadata = Record<string, {
  name: string; signature: string; debug_signature: string;
  source: string; start: SourcePos;
}>;
export type Place = { Local: number } | { Deref: number };
export type Graph = {
  capabilities: { place: Place; capability: "Read" | "Write" | "Exclusive" | "ShallowExclusive" }[];
  edges: { kind: "Borrow" | "Deref" | "BorrowFlow" }[];
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
  locals: { id: number; name: string | null }[];
  points: Point[];
  blocks: { block: number; successors: number[]; entry: Graph; exit: Graph }[];
  borrowck_source: string;
};
