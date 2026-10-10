use dctx0_core::{ReferenceKind, Source};
use dctx0_graph::{EdgeKind, build};
fn graph() -> dctx0_graph::SemanticGraph {
    let ir=dctx0_language::compile(&[Source{file:"spec.dctx0".into(),text:"workspace W { type A { b: Option<B> } type B { a: Option<A> } surface Api { operation get(a: A) -> B } constraint \"Respect this boundary\" }".into()}]).ir.unwrap();
    build(&ir)
}
#[test]
fn dependencies_users_cycles_and_explanations_are_deterministic() {
    let graph = graph();
    assert!(
        graph
            .dependencies("W.A")
            .iter()
            .any(|e| e.to == "W.B" && e.kind == EdgeKind::Dependency(ReferenceKind::UsesType))
    );
    assert!(graph.users("W.A").iter().any(|e| e.from == "W.Api.get"));
    assert!(
        graph
            .components()
            .iter()
            .any(|s| s.contains(&"W.A".into()) && s.contains(&"W.B".into()))
    );
    let impact = graph.impact("W.A");
    assert!(impact.iter().any(|i| i.subject == "W.Api.get"));
    assert_eq!(impact, graph.impact("W.A"));
    assert!(graph.impact("unknown").is_empty());
    for i in impact {
        assert_eq!(i.path.first().unwrap(), "W.A");
        assert_eq!(i.path.last().unwrap(), &i.subject);
        assert_eq!(i.path.len(), i.reasons.len() + 1);
    }
    let constraint = graph
        .nodes
        .keys()
        .find(|s| s.contains("#constraint"))
        .unwrap();
    assert!(
        graph
            .impact(constraint)
            .iter()
            .any(|i| i.subject == "W.Api.get")
    );
}
