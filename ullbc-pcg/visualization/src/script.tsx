import React from "react";
import { createRoot } from "react-dom/client";
import { loadAnalysis } from "./api";
import App from "./components/App";
import "./style.css";

const root = createRoot(document.getElementById("root")!);
root.render(<p>Loading ULLBC-PCG…</p>);
loadAnalysis().then(data => root.render(<App data={data} />)).catch(error => root.render(
  <div role="alert" className="error"><h2>Could not load analysis</h2><p>{String(error)}</p><p>Run scripts/run-case.sh, then reload.</p></div>
));
