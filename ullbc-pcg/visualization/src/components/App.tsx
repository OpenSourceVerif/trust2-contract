// Panel arrangement adapted from MIR-PCG AppInner.tsx (ISC).
import React, { useCallback, useEffect, useState } from "react";
import { Analysis, SourcePos } from "../types";
import SourceCodeViewer from "./SourceCodeViewer";
import UllbcGraph from "./UllbcGraph";
import PcgGraph from "./PcgGraph";
import PCGNavigator from "./PCGNavigator";

export default function App({ data }: { data: Analysis }) {
  const [selected, setSelected] = useState(() => Math.max(0, data.points.findIndex(p => p.steps.at(-1)!.graph.edges.some(e => e.kind === "Borrow"))));
  const [step, setStep] = useState(() => data.points[selected].steps.length - 1);
  const [minimized, setMinimized] = useState(false);
  const [fontSize, setFontSize] = useState(13);
  const [settings, setSettings] = useState(false);
  const [showActions, setShowActions] = useState(false);
  const [leftWidth, setLeftWidth] = useState(42);
  const [dragging, setDragging] = useState(false);
  const select = useCallback((index: number) => { setSelected(index); setStep(0); }, []);
  const advance = useCallback((forward: boolean) => {
    if (forward) {
      if (step + 1 < data.points[selected].steps.length) setStep(step + 1);
      else if (selected + 1 < data.points.length) { setSelected(selected + 1); setStep(0); }
    } else {
      if (step > 0) setStep(step - 1);
      else if (selected > 0) { setSelected(selected - 1); setStep(data.points[selected - 1].steps.length - 1); }
    }
  }, [selected, step, data.points]);
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLElement && (e.target.closest("input,select,textarea") || e.target.isContentEditable)) return;
      if (e.key === "q" || e.key === "a") { e.preventDefault(); advance(e.key === "a"); }
    };
    window.addEventListener("keydown", key); return () => window.removeEventListener("keydown", key);
  }, [advance]);
  useEffect(() => {
    if (!dragging) return;
    const move = (e: MouseEvent) => setLeftWidth(Math.min(65, Math.max(25, e.clientX / (window.innerWidth - 260) * 100)));
    const stop = () => setDragging(false);
    window.addEventListener("mousemove", move); window.addEventListener("mouseup", stop);
    return () => { window.removeEventListener("mousemove", move); window.removeEventListener("mouseup", stop); };
  }, [dragging]);
  const point = data.points[selected];
  const relative = (p: SourcePos) => ({ ...p, line: p.line - data.source_start_line });
  const span = point.span ? { low: relative(point.span.low), high: relative(point.span.high) } : null;
  const sourceClick = (position: SourcePos) => {
    const absolute = { ...position, line: position.line + data.source_start_line };
    const matches = data.points.flatMap((p, index) => p.span &&
      (absolute.line > p.span.low.line || (absolute.line === p.span.low.line && absolute.column >= p.span.low.column)) &&
      (absolute.line < p.span.high.line || (absolute.line === p.span.high.line && absolute.column < p.span.high.column)) ? [index] : []);
    if (matches.length) select(matches[(matches.indexOf(selected) + 1) % matches.length]);
  };
  const currentStep = point.steps[step];
  const file = currentStep.dot;
  const graph = currentStep.graph;
  return <div className="app">
    <main>
      <section className="left-panel" style={{ width: `${leftWidth}%` }}>
        <SourceCodeViewer functions={{ [data.name]: { name: data.name, signature: data.signature, debug_signature: data.signature, source: data.source, start: { line: data.source_start_line, column: 0 } } }}
          selectedFunction={data.name} onFunctionChange={() => select(0)} highlightSpan={span}
          onClickPosition={sourceClick} minimized={minimized} onToggleMinimized={() => setMinimized(!minimized)}
          fontSize={fontSize} onFontSizeChange={setFontSize} showSettings={settings} onToggleSettings={() => setSettings(!settings)} />
        {settings && <div className="settings"><label><input type="checkbox" checked={showActions} onChange={e => setShowActions(e.target.checked)} /> Show actions in ULLBC</label></div>}
        <div className="panel-label">ULLBC</div>
        <div className="ullbc"><UllbcGraph data={data} selected={selected} select={select} showActions={showActions} /></div>
      </section>
      <div className="divider" onMouseDown={() => setDragging(true)}><span /></div>
      <section className="graph-panel">
        <div className="panel-label">PCG · bb{point.location.block}[{point.location.statement_index}] · {currentStep.phase ?? "initial"}{currentStep.kind === "action" ? ` · action ${currentStep.action_index! + 1}` : ""}</div>
        <PcgGraph filename={file} />
        <div className="graph-footer">
          <code>{point.statement}</code><div>{currentStep.label}</div>
          <p>{graph.capabilities.length} capabilities · {graph.edges.filter(e => e.kind === "Borrow").length} active borrows</p>
          <div>E: exclusive · e: shallow exclusive · R: read · W: write · ∅: none</div>
          <div>Orange: borrow · Green: deref · Purple: reference flow · Blue: lifetime projection · Grey dashed: derived alias</div>
          <div>Node regions / locations: ULLBC · Borrow edge bw / '? IDs: rustc · Hover for details</div>
        </div>
      </section>
    </main>
    <PCGNavigator point={point} selectedStep={step} selectStep={setStep}
      previous={() => advance(false)} next={() => advance(true)} first={selected === 0 && step === 0}
      last={selected === data.points.length - 1 && step === point.steps.length - 1} graphFile={file} />
  </div>;
}
