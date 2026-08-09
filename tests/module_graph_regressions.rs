use avera_compiler::module::{detect_cycles, LoadedModule, ModuleDescriptor, ModuleGraph};
use std::path::PathBuf;

fn module(name: &str, deps: &[&str]) -> LoadedModule {
    LoadedModule {
        name: name.to_string(),
        descriptor: ModuleDescriptor {
            name: name.to_string(),
            depends: deps.iter().map(|dep| dep.to_string()).collect(),
            ..Default::default()
        },
        dir: PathBuf::from("."),
    }
}

#[test]
fn cycle_report_excludes_prefix_that_is_not_in_cycle() {
    let mut graph = ModuleGraph::new();
    graph.add(module("app.entry", &["app.a"]));
    graph.add(module("app.a", &["app.b"]));
    graph.add(module("app.b", &["app.a"]));

    let cycle = detect_cycles(&graph).expect("expected cycle");
    assert_eq!(cycle.len(), 2, "cycle should contain only the cycle itself");
    assert!(cycle.contains(&"app.a".to_string()));
    assert!(cycle.contains(&"app.b".to_string()));
    assert!(!cycle.contains(&"app.entry".to_string()));
}
