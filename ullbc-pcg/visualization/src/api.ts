import { Analysis } from "./types";

async function response(path: string): Promise<Response> {
  const result = await fetch(path, { cache: "no-store" });
  if (!result.ok) throw new Error(`${path}: HTTP ${result.status}`);
  return result;
}
export async function loadAnalysis(): Promise<Analysis> {
  const data = await (await response("data/analysis.json")).json() as Analysis;
  if (data.schema_version !== 3 || !Array.isArray(data.blocks) || !Array.isArray(data.points) || !data.points.length || data.points.some(p => !Array.isArray(p.steps) || !p.steps.length)) {
    throw new Error("Unsupported or empty ULLBC-PCG export. Regenerate with scripts/run-case.sh.");
  }
  return data;
}
export async function loadDot(filename: string): Promise<string> {
  // Export filenames are basenames, never remote resources or arbitrary paths.
  if (!/^bb\d+-\d+-(initial|(?:pre_operands|post_operands|pre_main|post_main)(?:-action-\d+)?)\.dot$/.test(filename)) throw new Error("Invalid graph filename");
  return (await response(`data/${filename}`)).text();
}
