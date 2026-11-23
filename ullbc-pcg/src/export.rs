//! Static viewer data. DOT is derived exclusively from computed graph snapshots.
use crate::*;
use charon_lib::{
    ast::{BodyBorrowckInfo, FunDecl, TranslatedCrate},
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
    pub blocks: Vec<BlockResult>,
    pub borrowck_source: String,
    pub analysis_stage: &'static str,
    pub function_id: charon_lib::ast::FunDeclId,
    pub body_fingerprint: String,
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
pub fn dot(graph: &Graph, locals: &[LocalInfo], facts: &BodyBorrowckInfo) -> String {
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
            Node::LifetimeProjection(p) => {
                // Keep the actual ULLBC reference/region identifiers. The phase
                // is detail of the snapshot, not another current-phase label.
                let mut label = projection_name(LifetimeProjection { label: None, ..p }, locals);
                if let Some(snapshot) = p.label {
                    label.push_str(&format!(" before {}", snapshot.location));
                }
                let region = facts
                    .reference_regions
                    .iter()
                    .find(|r| r.local == p.reference)
                    .expect("validated reference-region binding");
                let tooltip = format!(
                    "{}; ULLBC region '{}; rustc reference-type region '?{}",
                    projection_name(p, locals),
                    p.region.index(),
                    region.region
                );
                (
                    label,
                    format!(
                        "shape=octagon, color=blue, fontcolor=blue, tooltip={}",
                        quote(&tooltip)
                    ),
                )
            }
        };
        s.push_str(&format!(
            "  {} [label={}, {attrs}];\n",
            id(node),
            quote(&label)
        ));
    }
    for (index, edge) in graph.edges.iter().enumerate() {
        let (color, label, tooltip) = match edge {
            Edge::Borrow {
                location,
                borrow_kind,
                ..
            } => {
                let loan = facts
                    .loans
                    .iter()
                    .find(|l| {
                        l.reserve.block == location.block
                            && l.reserve.statement_index == location.statement_index
                    })
                    .expect("validated borrow creation point");
                let kind = if *borrow_kind == BorrowKind::Mutable {
                    "Default"
                } else {
                    "shared"
                };
                (
                    "orange",
                    format!("bw{}: {kind} '?{}", loan.id, loan.region),
                    format!("ULLBC borrow at {location}; loan and edge region IDs come from rustc"),
                )
            }
            Edge::Deref { .. } => (
                "green",
                String::new(),
                format!(
                    "{}; two arrows represent one Deref hyperedge",
                    edge_name(edge, locals)
                ),
            ),
            Edge::BorrowFlow { operand, .. } => {
                ("purple", format!("{operand:?}"), edge_name(edge, locals))
            }
        };
        for source in edge.sources() {
            s.push_str(&format!(
                "  {} -> {} [color={color}, label={}, tooltip={}, class=\"pcg-edge-{index}\"];\n",
                id(source),
                id(edge.target()),
                quote(&label),
                quote(&tooltip)
            ));
        }
        // Display-only relation, matching MIR-PCG's materialized alias edges.
        // An old reference value must not alias the current dereference node.
        if let Edge::Borrow {
            borrowed, assigned, ..
        } = edge
        {
            let deref = Node::Place(Place::Deref(assigned.reference));
            if assigned.base_label.is_none() && nodes.contains(&deref) {
                s.push_str(&format!(
                    "  {} -> {} [color=grey, style=dashed, tooltip={}];\n",
                    id(Node::Place(*borrowed)),
                    id(deref),
                    quote("Alias derived from the literal borrow; not an additional loan")
                ));
            }
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
    write_analysis(krate, &analyze_function(krate, function)?, output)
}

pub fn write_analysis(
    krate: &TranslatedCrate,
    result: &FunctionAnalysis,
    output: &Path,
) -> Result<ViewerData, Box<dyn std::error::Error>> {
    let function = krate
        .fun_decls
        .get(result.function_id)
        .ok_or("analysis function is missing")?;
    let input = function_input(krate, function)?;
    if result.analysis_stage != input.analysis_stage() {
        return Err("PCG result belongs to a different analysis stage".into());
    }
    let FunctionInput {
        body,
        facts,
        bundle,
    } = input;
    if result.body_fingerprint != charon_lib::ast::borrowck_body_fingerprint(body)
        || result.body_fingerprint != facts.body_fingerprint
    {
        return Err("PCG result does not match the current ULLBC".into());
    }
    let analysis = &result.analysis;
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
    for result in &analysis.points {
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
            steps: viewer_steps(result, &analysis.locals),
            action_labels: result
                .actions()
                .map(|a| action_label(&a.action, &analysis.locals))
                .collect(),
            result: result.clone(),
        });
    }
    let data = ViewerData {
        schema_version: 3,
        analysis_stage: result.analysis_stage,
        function_id: result.function_id,
        body_fingerprint: result.body_fingerprint.clone(),
        blocks: analysis.blocks.clone(),
        borrowck_source: format!("{} / {}", bundle.producer, bundle.stage),
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
        locals: analysis.locals.clone(),
        points,
    };
    fs::create_dir_all(output)?;
    fs::write(output.join("pcg.json"), serde_json::to_vec(result)?)?;
    for point in &data.points {
        for step in &point.steps {
            fs::write(
                output.join(&step.dot),
                dot(&step.graph, &data.locals, facts),
            )?;
        }
    }
    fs::write(
        output.join("analysis.json"),
        serde_json::to_string_pretty(&data)?,
    )?;
    Ok(data)
}
