// Adapted from MIR-PCG visualization/src/components/BasicBlockTable.tsx (ISC).
import React from "react";
import { Point } from "../types";

export default function BasicBlockTable({ block, entries, selected, select, showActions }: {
  block: number; entries: { point: Point; index: number }[];
  selected: number; select: (index: number) => void; showActions: boolean;
}) {
  return <table cellSpacing={0} cellPadding={4}
    style={{ borderCollapse: "collapse", width: "300px", boxShadow: "0 0 0 1px black", background: "white" }}>
    <tbody>
      <tr><td>(on start)</td><td><b>bb{block}</b></td></tr>
      {entries.map(({ point, index }, i) => <tr key={index}
        className={selected === index ? "highlight" : ""}
        data-statement={index} onClick={() => select(index)} title={point.statement}>
        <td>{i === entries.length - 1 ? "T" : point.location.statement_index}</td>
        <td><button className="statement-button" onClick={(event) => { event.stopPropagation(); select(index); }}
          aria-label={`bb${block}:${point.location.statement_index} ${point.statement.trim()}`}>
          <code>{point.statement}</code></button>
          {showActions && point.action_labels.length > 0 && <div style={{ marginTop: "4px", fontSize: "0.85em", fontStyle: "italic", fontFamily: "monospace", color: "#0066cc" }}>
            {point.action_labels.map((action, i) => <div key={i}>{action}</div>)}
          </div>}
        </td>
      </tr>)}
    </tbody>
  </table>;
}
