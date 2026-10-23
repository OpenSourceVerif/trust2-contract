//! Static viewer data. DOT is derived exclusively from computed graph snapshots.
use crate::*;
use charon_lib::{
    ast::{Body, FunDecl, TranslatedCrate},
    formatter::{AstFormatter, IntoFormatter},
    pretty::FmtWithCtx,
};
use serde::Serialize;
use std::{collections::BTreeSet, fs, path::Path};

#[derive(Serialize)]
pub struct SourcePos {
    pub line: usize,
    pub column: usize,
}
#[derive(Serialize)]
pub struct SourceSpan {
    pub low: SourcePos,
    pub high: SourcePos,
}
#[derive(Serialize)]
pub struct ViewerPoint {
    pub statement: String,
    pub span: Option<SourceSpan>,
    pub steps: Vec<ViewerStep>,
    pub action_labels: Vec<String>,
    #[serde(flatten)]
    pub result: PointResult,
}
#[derive(Serialize)]
pub struct ViewerStep {
    pub kind: &'static str,
    pub phase: Option<EvalStmtPhase>,
    pub action_index: Option<usize>,
    pub label: String,
    pub dot: String,
    pub graph: Graph,
}
#[derive(Serialize)]
pub struct ViewerData {
    pub schema_version: u32,
    pub name: String,
    pub signature: String,
    pub source: String,
    pub source_start_line: usize,
    pub locals: Vec<LocalInfo>,
    pub points: Vec<ViewerPoint>,
}

fn quote(s: &str) -> String {
    serde_json::to_string(s).unwrap()
}
pub fn place_name(p: Place, locals: &[LocalInfo]) -> String {
    let (Place::Local(id) | Place::Deref(id)) = p;
    let name = locals
        .iter()
        .find(|l| l.id == id)
        .and_then(|l| l.name.as_deref());
    let base = name
        .map(str::to_owned)
        .unwrap_or_else(|| format!("_{}", id.index()));
    if matches!(p, Place::Deref(_)) {
        format!("(*{base})")
    } else {
        base
    }
}
fn projection_name(p: LifetimeProjection, locals: &[LocalInfo]) -> String {
    let mut s = format!(
        "{}|'{}",
        place_name(Place::Local(p.reference), locals),
        p.region.index()
    );
    if let Some(label) = p.base_label {
        s.push_str(&format!(" [old @ {} {:?}]", label.location, label.phase));
    }
    if let Some(label) = p.label {
        s.push_str(&format!(" @ {} {:?}", label.location, label.phase));
    }
    s
}
fn node_name(node: Node, locals: &[LocalInfo]) -> String {
    match node {
        Node::Place(p) => place_name(p, locals),
        Node::LifetimeProjection(p) => projection_name(p, locals),
    }
}
fn edge_name(edge: &Edge, locals: &[LocalInfo]) -> String {
    let source = edge
        .sources()
        .iter()
        .map(|n| node_name(*n, locals))
        .collect::<Vec<_>>()
        .join(", ");
    let kind = match edge {
        Edge::Borrow {
            borrow_kind: BorrowKind::Mutable,
            ..
        } => "Borrow &mut",
        Edge::Borrow { .. } => "Borrow &",
        Edge::Deref { .. } => "Deref",
        Edge::BorrowFlow {
            operand: OperandKind::Copy,
            ..
        } => "BorrowFlow copy",
        Edge::BorrowFlow { .. } => "BorrowFlow move",
    };
    format!(
        "{kind}: {{{source}}} → {}",
        node_name(edge.target(), locals)
    )
}
pub fn dot(graph: &Graph, locals: &[LocalInfo]) -> String {
    let mut s =
        String::from("digraph PCG {\n  rankdir=TB;\n  node [shape=rect, fontname=Courier];\n");
    let mut nodes: BTreeSet<Node> = graph
        .capabilities
        .iter()
        .map(|c| Node::Place(c.place))
        .collect();
    for e in &graph.edges {
        nodes.extend(e.sources());
        nodes.insert(e.target());
    }
    let nodes: Vec<_> = nodes.into_iter().collect();
    let id = |n| format!("n{}", nodes.iter().position(|p| *p == n).unwrap());
    for &node in &nodes {
        let (label, attrs) = match node {
            Node::Place(place) => {
                let cap = graph.capability(place);
                let cap_name = cap.map(|c| format!("{c:?}")).unwrap_or_else(|| "∅".into());
                let color = if matches!(cap, None | Some(Capability::Write)) {
                    "gray"
                } else if matches!(place, Place::Deref(_)) {
                    "darkgreen"
                } else {
                    "black"
                };
                let style = if matches!(place, Place::Deref(_)) {
                    ", style=rounded, penwidth=1.5"
                } else {
                    ""
                };
                (
                    format!("{}: {}", place_name(place, locals), cap_name),
                    format!("color={color}, fontcolor={color}{style}"),
                )
            }
            Node::LifetimeProjection(p) => (
                projection_name(p, locals).replace(" @ ", "\n@ "),
                "shape=octagon, color=blue, fontcolor=blue".into(),
            ),
        };
        s.push_str(&format!(
            "  {} [label={}, {attrs}];\n",
            id(node),
            quote(&label)
        ));
    }
    for (index, edge) in graph.edges.iter().enumerate() {
        let color = match edge {
            Edge::Borrow { .. } => "orange",
            Edge::Deref { .. } => "green",
            Edge::BorrowFlow { .. } => "purple",
        };
        let label = match edge {
            Edge::Borrow {
                location,
                borrow_kind,
                ..
            } => format!(
                "{} @ {location}",
                if *borrow_kind == BorrowKind::Mutable {
                    "&mut"
                } else {
                    "&"
                }
            ),
            Edge::Deref { .. } => "deref".into(),
            Edge::BorrowFlow { operand, .. } => format!("{operand:?}"),
        };
        let sources = edge.sources();
        if sources.len() > 1 {
            // Preserve the deref hyperedge; do not draw two independent relations.
            s.push_str(&format!("  h{index} [shape=point, color={color}];\n"));
            for source in sources {
                s.push_str(&format!(
                    "  {} -> h{index} [color={color}, arrowhead=none];\n",
                    id(source)
                ));
            }
            s.push_str(&format!(
                "  h{index} -> {} [color={color}, label={}];\n",
                id(edge.target()),
                quote(&label)
            ));
        } else {
            s.push_str(&format!(
                "  {} -> {} [color={color}, label={}];\n",
                id(sources[0]),
                id(edge.target()),
                quote(&label)
            ));
        }
    }
    s.push_str("}\n");
    s
}
pub fn action_label(action: &Action, locals: &[LocalInfo]) -> String {
    match action {
        Action::Require { place, capability } => {
            format!("Require {}: {capability:?}", place_name(*place, locals))
        }
        Action::SetCapability {
            place,
            from,
            to,
            reason,
        } => {
            let cap =
                |c: &Option<Capability>| c.map(|k| format!("{k:?}")).unwrap_or_else(|| "∅".into());
            format!(
                "{}: {} → {} ({reason:?})",
                place_name(*place, locals),
                cap(from),
                cap(to)
            )
        }
        Action::Allocate { local } => {
            format!("Allocate {}: W", place_name(Place::Local(*local), locals))
        }
        Action::Deallocate { local } => {
            format!("Deallocate {}", place_name(Place::Local(*local), locals))
        }
        Action::AddEdge { edge } => format!("Add {}", edge_name(edge, locals)),
        Action::RemoveEdge { edge } => format!("Remove {}", edge_name(edge, locals)),
        Action::LabelProjection { from, to } => format!(
            "Relabel {} → {}",
            projection_name(*from, locals),
            projection_name(*to, locals)
        ),
    }
}
fn viewer_steps(result: &PointResult, locals: &[LocalInfo]) -> Vec<ViewerStep> {
    let mut steps = Vec::new();
    let prefix = format!(
        "bb{}-{}",
        result.location.block.index(),
        result.location.statement_index
    );
    steps.push(ViewerStep {
        kind: "initial",
        phase: None,
        action_index: None,
        label: "initial".into(),
        dot: format!("{prefix}-initial.dot"),
        graph: result.initial.clone(),
    });
    for phase in &result.phases {
        let name = serde_json::to_value(phase.phase)
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned();
        for (index, action) in phase.actions.iter().enumerate() {
            steps.push(ViewerStep {
                kind: "action",
                phase: Some(phase.phase),
                action_index: Some(index),
                label: action_label(&action.action, locals),
                dot: format!("{prefix}-{name}-action-{index}.dot"),
                graph: action.after.clone(),
            });
        }
        steps.push(ViewerStep {
            kind: "phase",
            phase: Some(phase.phase),
            action_index: None,
            label: name.clone(),
            dot: format!("{prefix}-{name}.dot"),
            graph: phase.after.clone(),
        });
    }
    steps
}

/// Export only after the complete function successfully passes analysis.
pub fn write(
    krate: &TranslatedCrate,
    function: &FunDecl,
    output: &Path,
) -> Result<ViewerData, Box<dyn std::error::Error>> {
    let Body::Unstructured(body) = &function.body else {
        return Err("expected an ULLBC body".into());
    };
    let analysis = analyze(body)?;
    let fmt = krate.into_fmt();
    let local_fmt = fmt.set_locals(&body.locals);
    let span = function.item_meta.span.data;
    let source = krate
        .files
        .get(span.file_id)
        .and_then(|file| file.contents.as_ref())
        .map(|s| {
            s.lines()
                .skip(span.beg.line.saturating_sub(1))
                .take(span.end.line.saturating_sub(span.beg.line) + 1)
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    let mut points = Vec::new();
    for result in analysis.points {
        let location = result.location;
        let block = &body.body[location.block];
        let (statement, stmt_span) =
            if let Some(stmt) = block.statements.get(location.statement_index) {
                (stmt.to_string_with_ctx(&local_fmt), stmt.span.data)
            } else {
                (
                    block.terminator.to_string_with_ctx(&local_fmt),
                    block.terminator.span.data,
                )
            };
        let span = (stmt_span.file_id == span.file_id).then_some(SourceSpan {
            low: SourcePos {
                line: stmt_span.beg.line,
                column: stmt_span.beg.col,
            },
            high: SourcePos {
                line: stmt_span.end.line,
                column: stmt_span.end.col,
            },
        });
        points.push(ViewerPoint {
            statement,
            span,
            steps: viewer_steps(&result, &analysis.locals),
            action_labels: result
                .actions()
                .map(|a| action_label(&a.action, &analysis.locals))
                .collect(),
            result,
        });
    }
    let data = ViewerData {
        schema_version: 2,
        name: function.item_meta.name.to_string_with_ctx(&fmt),
        signature: format!(
            "fn {}({}) -> {}",
            function.item_meta.name.to_string_with_ctx(&fmt),
            function
                .signature
                .inputs
                .iter()
                .map(|ty| ty.to_string_with_ctx(&fmt))
                .collect::<Vec<_>>()
                .join(", "),
            function.signature.output.to_string_with_ctx(&fmt)
        ),
        source,
        source_start_line: span.beg.line,
        locals: analysis.locals,
        points,
    };
    fs::create_dir_all(output)?;
    for point in &data.points {
        for step in &point.steps {
            fs::write(output.join(&step.dot), dot(&step.graph, &data.locals))?;
        }
    }
    fs::write(
        output.join("analysis.json"),
        serde_json::to_string_pretty(&data)?,
    )?;
    Ok(data)
}
