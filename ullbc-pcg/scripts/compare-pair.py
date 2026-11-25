#!/usr/bin/env python3
"""Compare the observable PCG semantics of fixtures/projected_borrows.rs::change."""
import argparse
import json
from pathlib import Path


def read(path):
    return json.loads(path.read_text())


def unique(items):
    item, = items
    return item


def main():
    if not __debug__:
        raise SystemExit("Run without -O to enable the comparison assertions.")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mir", type=Path, help="MIR-PCG function directory containing mir.json")
    parser.add_argument("ullbc", type=Path, help="ULLBC-PCG directory containing analysis.json")
    args = parser.parse_args()
    mir = read(args.mir / "mir.json")
    trace = read(args.mir / "pcg_data.json")
    ullbc = read(args.ullbc / "analysis.json")
    local = unique(x for x in ullbc["locals"] if x["name"] == "p")
    owner = {"Local": local["id"]}
    fields = {f["name"]: {"Field": {"local": local["id"], "projection": i}}
              for i, f in enumerate(local["fields"])}
    ref = unique(x["id"] for x in ullbc["locals"] if x["name"] == "r")
    mir_points = [(b, i, stmt) for b in mir["nodes"] for i, stmt in enumerate(b["stmts"])]
    for title, search, phase in (("borrow", "&mut", "pre_operands"),
                                 ("borrowed", "&mut", "post_main"),
                                 ("write", "const 7", "pre_main"),
                                 ("return pair", "move p", "pre_operands")):
        b, i, src = unique(row for row in mir_points if search in row[2]["stmt"])
        u = unique(p for p in ullbc["points"] if search in p["statement"])
        if src["span"]["low"]["line"] != u["span"]["low"]["line"]:
            raise ValueError(f"source lines disagree at {title}")
        m = read(args.mir / f"{b['id']}_stmt_{i}_{phase}.json")["objects"]
        f = unique(f for f in u["phases"] if f["phase"] == phase)
        ma = [a["action"]["data"]["kind"] for a in trace[b["id"]]["statements"][i]["actions"][phase]]
        ua = [a["action"] for a in f["actions"]]
        caps = {n["label"]: n.get("cap") for n in m["nodes"].values() if n["kind"] == "place"}
        graph = f["after"]

        def cap(place):
            return next((c["capability"] for c in graph["capabilities"] if c["place"] == place), None)

        if title == "borrow":
            assert any(a["type"] == "Expand" and a["data"]["from"] == "p" for a in ma)
            assert any(a.get("Expand", {}).get("base") == owner for a in ua)
        elif title == "borrowed":
            assert caps["p.x"] is None and caps["p.y"] == "E"
            assert cap(fields["x"]) is None and cap(fields["y"]) == "Exclusive"
            assert any(e["kind"] == "borrow" and m["nodes"][e["source"]]["label"] == "p.x"
                       and m["nodes"][e["target"]]["label"].startswith("r↓")
                       for e in m["edges"].values())
            assert any(e["kind"] == "Borrow" and e["borrowed"] == fields["x"]
                       and e["assigned"]["reference"] == ref for e in graph["edges"])
        elif title == "write":
            assert caps["*r"] == "W" and cap({"Deref": ref}) == "Write"
            assert any(a["type"] == "AddEdge" and "*r" in str(a["data"]) for a in ma)
            assert any(a.get("AddEdge", {}).get("edge", {}).get("kind") == "Deref" for a in ua)
        else:
            mir_events = []
            for a in ma:
                if a["type"] == "RemoveEdge":
                    mir_events.append("borrow" if "borrow:" in a["data"] else "deref")
                elif a["type"] == "RegainLoanedCapability" and a["data"]["place"] == "p.x":
                    mir_events.append("regain")
                elif a["type"] == "Collapse" and a["data"]["to"] == "p":
                    mir_events.append("collapse")
            ullbc_events = []
            for a in ua:
                if "RemoveEdge" in a:
                    ullbc_events.append(a["RemoveEdge"]["edge"]["kind"].lower())
                elif a.get("SetCapability", {}).get("place") == fields["x"] and a["SetCapability"]["to"] == "Exclusive":
                    ullbc_events.append("regain")
                elif a.get("Collapse", {}).get("base") == owner:
                    ullbc_events.append("collapse")
            assert mir_events == ullbc_events == ["deref", "borrow", "regain", "collapse"]
            assert caps["p"] == "E" and cap(owner) == "Exclusive"
        print(f"{title}: MIR {b['id']}[{i}], ULLBC bb{u['location']['block']}[{u['location']['statement_index']}] · {phase}")


if __name__ == "__main__":
    main()
