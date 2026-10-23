// Rendering approach adapted from MIR-PCG PcgGraph.tsx (ISC).
import React, { useEffect, useRef, useState } from "react";
import { instance } from "@viz-js/viz";
import panzoom from "panzoom";
import { loadDot } from "../api";

const renderer = instance();
export default function PcgGraph({ filename }: { filename: string }) {
  const container = useRef<HTMLDivElement>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let cancelled = false;
    let dispose: (() => void) | undefined;
    setError(null);
    container.current?.replaceChildren();
    Promise.all([loadDot(filename), renderer]).then(([dot, viz]) => {
      if (cancelled || !container.current) return;
      const svg = viz.renderSVGElement(dot);
      svg.setAttribute("role", "img"); svg.setAttribute("aria-label", "Computed place capability graph");
      svg.style.width = "auto"; svg.style.height = "auto";
      svg.style.maxWidth = "95%"; svg.style.maxHeight = "95%";
      container.current.replaceChildren(svg);
      const zoom = panzoom(svg, { maxZoom: 10, minZoom: 0.1, bounds: true, boundsPadding: 0.1 });
      dispose = () => zoom.dispose();
    }).catch(e => { if (!cancelled) setError(String(e)); });
    return () => { cancelled = true; dispose?.(); };
  }, [filename]);
  return <><div id="pcg-graph" ref={container} style={{ flex: 1, minHeight: 0, overflow: "hidden" }} />
    {error && <p role="alert" className="error">{error}</p>}</>;
}
