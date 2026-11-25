# PCG workflow report

Open `pcg-to-viper-cases.html` directly, or run from this directory:

```sh
python3 -m http.server 8768 --bind 127.0.0.1
```

Browse <http://localhost:8768/pcg-to-viper-cases.html>.

Keep this HTML file together with its `assets/` and `evidence/` directories.
All report links and resources resolve within this directory tree.
Upstream GitHub links are optional; frozen source copies are included locally.

Absolute paths inside original logs and run commands describe the historical
execution environment. The report does not load files through those paths.

## Resource annotations

Open `pcg-resource-contracts.html` for 23 evidence-backed annotation cases.
Each case occupies one 16:9 slide with offline syntax highlighting.
Use arrow keys or the toolbar to navigate; use the evidence button for details.
The Rocq-style annotations are explanatory templates, not checked Rocq proofs.
Evidence and derivations are under `evidence/pcg-resource-rules/`.

Rebuild with Python 3 and an existing Graphviz `dot` executable on PATH:

```sh
python3 assets/build_pcg_resource_rules.py
python3 assets/check_pcg_resource_rules.py
```

The builder reads the original report and frozen evidence in this directory.
The HTML itself needs no build tools, server, or network connection.
