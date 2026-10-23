// Layout/components adapted from MIR-PCG MirGraph and ReactFlowBasicBlockNode (ISC).
import React from "react";
import ReactFlow, { Background, Controls, Handle, Position, NodeProps, MarkerType } from "reactflow";
import "reactflow/dist/style.css";
import { Analysis, Point } from "../types";
import BasicBlockTable from "./BasicBlockTable";

type BlockData = { block: number; entries: { point: Point; index: number }[]; selected: number; select: (i: number) => void; showActions: boolean };
function Block({ data }: NodeProps<BlockData>) {
  return <div className="nopan" style={{ pointerEvents: "all" }}>
    <BasicBlockTable {...data} />
    <Handle type="target" position={Position.Top} style={{ visibility: "hidden" }} />
    <Handle type="source" position={Position.Bottom} style={{ visibility: "hidden" }} />
  </div>;
}
const nodeTypes = { basicBlock: Block };
export default function UllbcGraph({ data, selected, select, showActions }: { data: Analysis; selected: number; select: (i: number) => void; showActions: boolean }) {
  const blocks = new Map<number, BlockData["entries"]>();
  data.points.forEach((point, index) => {
    const entries = blocks.get(point.location.block) ?? [];
    entries.push({ point, index }); blocks.set(point.location.block, entries);
  });
  let y = 0;
  const nodes = [...blocks].map(([block, entries]) => {
    const node = { id: String(block), type: "basicBlock", position: { x: 40, y },
      data: { block, entries, selected, select, showActions }, draggable: false };
    y += 90 + entries.reduce((height, e) => height + 32 + (showActions ? e.point.action_labels.length * 15 : 0), 0);
    return node;
  });
  // The accepted fragment is a single goto chain; order is the actual traversal.
  const edges = nodes.slice(1).map((node, i) => ({ id: `edge-${i}`, source: nodes[i].id, target: node.id, markerEnd: { type: MarkerType.ArrowClosed } }));
  return <ReactFlow nodes={nodes} edges={edges} nodeTypes={nodeTypes} fitView
    minZoom={0.3} maxZoom={2} nodesConnectable={false} fitViewOptions={{ padding: 0.15, maxZoom: 1.2 }}>
    <Background /><Controls showInteractive={false} />
  </ReactFlow>;
}
