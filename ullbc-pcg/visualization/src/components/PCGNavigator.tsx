// Controls and colors adapted from MIR-PCG PCGNavigator.tsx (ISC).
import React, { useEffect, useRef } from "react";
import { Point } from "../types";

export default function PCGNavigator({ point, selectedStep, selectStep, previous, next, first, last, graphFile }: {
  point: Point; selectedStep: number; selectStep: (index: number) => void;
  previous: () => void; next: () => void; first: boolean; last: boolean; graphFile: string;
}) {
  const active = useRef<HTMLButtonElement>(null);
  useEffect(() => { active.current?.scrollIntoView({ block: "nearest" }); }, [point, selectedStep]);
  return <aside className="navigator">
    <div className="navigator-heading"><b>Navigator</b><div><code>bb{point.location.block}[{point.location.statement_index}]</code></div></div>
    <div style={{ flex: 1, padding: "15px", overflow: "auto" }}>
      {point.steps.map((step, index) => <button key={index} ref={selectedStep === index ? active : null}
        className={step.kind === "action" ? "action action-button" : "phase"}
        aria-pressed={selectedStep === index} title={step.kind === "action" ? `${step.phase} · after action ${step.action_index! + 1}` : step.label}
        onClick={() => selectStep(index)}>
        {step.kind === "action" ? <code>{step.label}</code> : step.label}
      </button>)}
    </div>
    <div style={{ padding: "0 15px 10px" }}>
      <p className="live-label">Live references after statement: {point.live_references_after.length ? point.live_references_after.map(id => `_${id}`).join(", ") : "∅"}</p>
      <div style={{ display: "flex", gap: 6 }}>
        <button disabled={first} onClick={previous}>← Previous</button>
        <button disabled={last} onClick={next}>Next →</button>
      </div>
    </div>
    <a className="open-graph" href={`data/${graphFile}`} target="_blank" rel="noreferrer">Open Current DOT</a>
    <div className="navigator-footer">Press ‘q’ / ‘a’ to navigate phases and actions</div>
  </aside>;
}
