# MIR-PCG visualization attribution

The viewer reuses and adapts files from the local MIR-PCG checkout,
`pcg/visualization`, at commit `6fdb05e6377da5316e5388320d2ddf3d13082f6e`.
Its `package.json` declares the ISC license. Original authors retain their rights.

- `index.html`: original table/highlight/global styles, title changed.
- `webpack.config.cjs`: copied build configuration.
- `src/components/SourceCodeViewer.tsx`: copied source viewer.
- `BasicBlockTable.tsx`, `UllbcGraph.tsx`, `PcgGraph.tsx`, `PCGNavigator.tsx`,
  `App.tsx`, `style.css`: adapted presentation, controls, colors and graph rendering
  from their corresponding MIR-PCG components (`MirGraph`, `AppInner`, etc.).

The ULLBC adapter and export implementation are local. The Rust analyzer now
ports a supported subset of the upstream four-phase rules; source correspondence
and limitations are documented in ../ALGORITHM.md. The viewer consumes only this
analyzer's JSON/DOT, including actual per-action snapshots; it does not consume
MIR-PCG analysis results.

ISC license text:

Permission to use, copy, modify, and/or distribute this software for any purpose
with or without fee is hereby granted, provided that the above copyright notice
and this permission notice appear in all copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES WITH
REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF MERCHANTABILITY
AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY SPECIAL, DIRECT,
INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES WHATSOEVER RESULTING FROM
LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR
OTHER TORTIOUS ACTION, ARISING OUT OF OR IN CONNECTION WITH THE USE OR
PERFORMANCE OF THIS SOFTWARE.
