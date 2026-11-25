import { Analysis } from "./types";

export function dataUrl(filename: string): string {
  const source = new URLSearchParams(window.location.search).get("datasrc") ?? "data";
  if (source !== "data" && !/^\/data\/runs\/[a-f0-9]{32}$/.test(source)) {
    throw new Error("Invalid ULLBC-PCG result directory");
  }
  if (filename !== "analysis.json" && !/^bb\d+-\d+-(initial|(?:pre_operands|post_operands|pre_main|post_main)(?:-action-\d+)?)\.dot$/.test(filename)) {
    throw new Error("Invalid graph filename");
  }
  return `${source}/${filename}`;
}

async function response(path: string): Promise<Response> {
  const result = await fetch(path, { cache: "no-store" });
  if (!result.ok) throw new Error(`${path}: HTTP ${result.status}`);
  return result;
}
export async function loadAnalysis(): Promise<Analysis> {
  const data = await (await response(dataUrl("analysis.json"))).json() as Analysis;
  if (![3, 4].includes(data.schema_version) || !Array.isArray(data.blocks) || !Array.isArray(data.points) || !data.points.length || data.points.some(p => !Array.isArray(p.steps) || !p.steps.length)) {
    throw new Error("Unsupported or empty ULLBC-PCG export. Regenerate with scripts/run-case.sh.");
  }
  return data;
}
export async function loadDot(filename: string): Promise<string> {
  // Export filenames are basenames, never remote resources or arbitrary paths.
  if (!/^bb\d+-\d+-(initial|(?:pre_operands|post_operands|pre_main|post_main)(?:-action-\d+)?)\.dot$/.test(filename)) throw new Error("Invalid graph filename");
  return (await response(dataUrl(filename))).text();
}
