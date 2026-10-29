// Controls and colors adapted from MIR-PCG PCGNavigator.tsx (ISC).
import React, { useEffect, useRef } from "react";
import { Point } from "../types";
import { dataUrl } from "../api";

export default function PCGNavigator({ point, selectedStep, selectStep, previous, next, first, last, graphFile }: {
  point: Point; selectedStep: number; selectStep: (index: number) => void;
  previous: () => void; next: () => void; first: boolean; last: boolean; graphFile: string;
}) {
  const active = useRef<HTMLButtonElement>(null);
  useEffect(() => { active.current?.scrollIntoView({ block: "nearest" }); }, [point, selectedStep]);
  const currentPhase = point.steps[selectedStep].phase;
  const queries = point.phases.find(p => p.phase === currentPhase)?.borrowck_queries ?? [];
  const loanSet = (ids: number[]) => ids.length ? ids.map(id => `L${id}`).join(", ") : "∅";
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
      <p className="live-label">NLL loans before / after statement: {loanSet(point.loans_before)} / {loanSet(point.loans_after)}</p>
      {queries.length > 0 && <details><summary>Borrow queries in this phase ({queries.length})</summary>
        {queries.map((q, i) => <div key={i}><code>{"LoanInScope" in q
          ? `in_scope(L${q.LoanInScope.loan}) = ${q.LoanInScope.result}`
          : `outlives(_${q.Outlives.from}, _${q.Outlives.to}) = ${q.Outlives.result}`}</code></div>)}
      </details>}
      <div style={{ display: "flex", gap: 6 }}>
        <button disabled={first} onClick={previous}>← Previous</button>
        <button disabled={last} onClick={next}>Next →</button>
      </div>
    </div>
    <a className="open-graph" href={dataUrl(graphFile)} target="_blank" rel="noreferrer">Open Current DOT</a>
    <div className="navigator-footer">Press ‘q’ / ‘a’ to navigate phases and actions</div>
  </aside>;
}
