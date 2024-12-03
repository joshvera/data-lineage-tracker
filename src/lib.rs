use std::collections::HashMap;
use std::error::Error;
use std::path::Path;
use tree_sitter_graph::graph::Value;
use tree_sitter_graph::graph::{Graph, GraphNodeRef};
use tree_sitter_graph::Identifier;

pub struct DataLineageTracker<'tree> {
    graph: Graph<'tree>,
    tree: Option<tree_sitter::Tree>,
    source_code: String,
}

impl<'tree> DataLineageTracker<'tree> {
    pub fn new() -> Self {
        Self {
            graph: Graph::new(),
            tree: None,
            source_code: String::new(),
        }
    }

    pub fn analyze_file<P: AsRef<Path>>(&'tree mut self, path: P) -> Result<&'tree Self, Box<dyn Error>> {
        self.source_code = std::fs::read_to_string(path)?;

        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(tree_sitter_javascript::language())
            .map_err(|e| Box::new(e) as Box<dyn Error>)?;

        // Store the tree in the struct
        self.tree = Some(parser
            .parse(&self.source_code, None)
            .ok_or("Failed to parse source code")?);

        // Create a new graph
        let mut new_graph = Graph::new();
        let root_node = new_graph.add_graph_node();
        let root_node_mut = &mut new_graph[root_node];
        root_node_mut
            .attributes
            .add(Identifier::from("type"), Value::from("root"))
            .ok();

        // Get the root node from the stored tree
        if let Some(tree) = &self.tree {
            let root_node_ref = tree.root_node();
            
            // Process the tree with the new graph
            self.process_tree(root_node_ref, &mut new_graph, &root_node)?;

            // After processing is complete, assign the new graph
            self.graph = new_graph;

            println!("Generated Graph:");
            println!("{}", self.graph.pretty_print());
        }

        Ok(self)
    }

    fn process_tree<'a>(
        &self,
        node: tree_sitter::Node<'a>,
        graph: &mut Graph<'a>,
        parent_ref: &GraphNodeRef,
    ) -> Result<(), Box<dyn Error>> {
        // Create scope node if this is a scope-defining node
        let current_scope_ref = match node.kind() {
            "function_declaration" | "method_definition" | "class_declaration" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    if let Ok(name) = name_node.utf8_text(self.source_code.as_bytes()) {
                        let scope_node = graph.add_graph_node();
                        let parent_node = &mut graph[*parent_ref];
                        parent_node.add_edge(scope_node).ok();

                        let scope_node_mut = &mut graph[scope_node];
                        scope_node_mut
                            .attributes
                            .add(Identifier::from("type"), Value::from("scope"))
                            .ok();
                        scope_node_mut
                            .attributes
                            .add(Identifier::from("name"), Value::from(name))
                            .ok();
                        
                        scope_node
                    } else {
                        *parent_ref
                    }
                } else {
                    *parent_ref
                }
            }
            _ => *parent_ref
        };

        // Process variable declarations and references
        match node.kind() {
            "variable_declarator" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    if let Ok(name) = name_node.utf8_text(self.source_code.as_bytes()) {
                        let scope = self.determine_scope(&node);
                        let start_pos = name_node.start_position();
                        
                        let decl_node = graph.add_graph_node();
                        let parent_node = &mut graph[current_scope_ref];
                        parent_node.add_edge(decl_node).ok();

                        let decl_node_mut = &mut graph[decl_node];
                        decl_node_mut
                            .attributes
                            .add(Identifier::from("type"), Value::from("declaration"))
                            .ok();
                        decl_node_mut
                            .attributes
                            .add(Identifier::from("name"), Value::from(name))
                            .ok();
                        decl_node_mut
                            .attributes
                            .add(Identifier::from("scope"), Value::from(scope))
                            .ok();
                        decl_node_mut
                            .attributes
                            .add(
                                Identifier::from("position"),
                                Value::from(format!("{},{}", start_pos.row + 1, start_pos.column + 1))
                            )
                            .ok();
                    }
                }
            }
            "identifier" => {
                if let Ok(name) = node.utf8_text(self.source_code.as_bytes()) {
                    let scope = self.determine_scope(&node);
                    let start_pos = node.start_position();

                    let ref_node = graph.add_graph_node();
                    let parent_node = &mut graph[current_scope_ref];
                    parent_node.add_edge(ref_node).ok();

                    let ref_node_mut = &mut graph[ref_node];
                    ref_node_mut
                        .attributes
                        .add(Identifier::from("type"), Value::from("reference"))
                        .ok();
                    ref_node_mut
                        .attributes
                        .add(Identifier::from("name"), Value::from(name))
                        .ok();
                    ref_node_mut
                        .attributes
                        .add(Identifier::from("scope"), Value::from(scope))
                        .ok();
                    ref_node_mut
                        .attributes
                        .add(
                            Identifier::from("position"),
                            Value::from(format!("{},{}", start_pos.row + 1, start_pos.column + 1))
                        )
                        .ok();
                }
            }
            _ => {}
        }

        // Traverse children using simple iteration
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.process_tree(child, graph, &current_scope_ref)?;
        }

        Ok(())
    }

    fn determine_scope(&self, node: &tree_sitter::Node) -> String {
        let mut current = node.parent();
        let mut scope_parts = Vec::new();

        while let Some(n) = current {
            match n.kind() {
                "function_declaration" | "method_definition" | "class_declaration" => {
                    if let Some(name_node) = n.child_by_field_name("name") {
                        if let Ok(name) = name_node.utf8_text(self.source_code.as_bytes()) {
                            scope_parts.push(name.to_string());
                        }
                    }
                }
                _ => {}
            }
            current = n.parent();
        }

        scope_parts.reverse();
        if scope_parts.is_empty() {
            "global".to_string()
        } else {
            scope_parts.join("::")
        }
    }

    pub fn get_full_lineage(&self, variable_name: &str) -> Vec<String> {
        let mut lineage = Vec::new();

        // Find declaration node for the variable
        for node_ref in self.graph.iter_nodes() {
            let node = &self.graph[node_ref];
            if let (Some(node_type), Some(name)) = (
                node.attributes
                    .get(&Identifier::from("type"))
                    .map(|v| v.as_str().unwrap_or("")),
                node.attributes
                    .get(&Identifier::from("name"))
                    .map(|v| v.as_str().unwrap_or("")),
            ) {
                if node_type == "declaration" && name == variable_name {
                    if let Some(scope) = node.attributes.get(&Identifier::from("scope")) {
                        lineage.push(format!(
                            "Declared in scope: {}",
                            scope.as_str().unwrap_or("")
                        ));
                    }
                }
            }
        }

        // Find all references to the variable
        for node_ref in self.graph.iter_nodes() {
            let node = &self.graph[node_ref];
            if let (Some(node_type), Some(name)) = (
                node.attributes
                    .get(&Identifier::from("type"))
                    .map(|v| v.as_str().unwrap_or("")),
                node.attributes
                    .get(&Identifier::from("name"))
                    .map(|v| v.as_str().unwrap_or("")),
            ) {
                if node_type == "reference" && name == variable_name {
                    if let Some(scope) = node.attributes.get(&Identifier::from("scope")) {
                        lineage.push(format!(
                            "Referenced in scope: {}",
                            scope.as_str().unwrap_or("")
                        ));
                    }
                }
            }
        }

        lineage
    }

    pub fn print_lineage(&self) {
        println!("Variable Declarations and References:");
        println!("===================================");

        let mut declarations: HashMap<String, (String, Vec<(usize, usize, String)>)> =
            HashMap::new();

        // Find all declarations
        for node_ref in self.graph.iter_nodes() {
            let node = &self.graph[node_ref];
            if let (Some(node_type), Some(name), Some(scope)) = (
                node.attributes
                    .get(&Identifier::from("type"))
                    .map(|v| v.as_str().unwrap_or("")),
                node.attributes
                    .get(&Identifier::from("name"))
                    .map(|v| v.as_str().unwrap_or("")),
                node.attributes
                    .get(&Identifier::from("scope"))
                    .map(|v| v.as_str().unwrap_or("")),
            ) {
                if node_type == "declaration" {
                    if let Some(pos) = node.attributes.get(&Identifier::from("position")) {
                        let _pos_parts = pos
                            .as_str()
                            .unwrap_or("0,0")
                            .split(',')
                            .map(|s| s.parse::<usize>().unwrap_or(0))
                            .collect::<Vec<_>>();
                        declarations.insert(name.to_string(), (scope.to_string(), Vec::new()));
                    }
                }
            }
        }

        // Find all references
        for node_ref in self.graph.iter_nodes() {
            let node = &self.graph[node_ref];
            if let (Some(node_type), Some(name), Some(scope)) = (
                node.attributes
                    .get(&Identifier::from("type"))
                    .map(|v| v.as_str().unwrap_or("")),
                node.attributes
                    .get(&Identifier::from("name"))
                    .map(|v| v.as_str().unwrap_or("")),
                node.attributes
                    .get(&Identifier::from("scope"))
                    .map(|v| v.as_str().unwrap_or("")),
            ) {
                if node_type == "reference" {
                    if let Some(pos) = node.attributes.get(&Identifier::from("position")) {
                        let start_pos = pos
                            .as_str()
                            .unwrap_or("0,0")
                            .split(',')
                            .map(|s| s.parse::<usize>().unwrap_or(0))
                            .collect::<Vec<_>>();
                        if let Some((_, refs)) = declarations.get_mut(name) {
                            refs.push((start_pos[0], start_pos[1], scope.to_string()));
                        }
                    }
                }
            }
        }

        // Print the collected information
        for (name, (scope, references)) in declarations {
            println!("\nVariable: {}", name);
            println!(" Declared in scope: {}", scope);
            if references.is_empty() {
                println!(" No references found");
            } else {
                println!(" References:");
                for (line, column, ref_scope) in references {
                    println!(
                        " - At line {}, column {} (in scope: {})",
                        line, column, ref_scope
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_variable_tracking() -> Result<(), Box<dyn std::error::Error>> {
        let source_code = r#"
        const globalVar = 42;
        function outer() {
            let outerVar = globalVar + 1;
            function inner() {
                const innerVar = outerVar * 2;
                return innerVar;
            }
            return inner() + outerVar;
        }
        class Example {
            constructor() {
                this.classVar = globalVar;
            }
            method() {
                return this.classVar + globalVar;
            }
        }
        "#;

        let temp_file = NamedTempFile::new()?;
        write(&temp_file, source_code)?;

        let mut tracker = DataLineageTracker::new();
        tracker.analyze_file(temp_file.path())?;

        let lineage = tracker.get_full_lineage("globalVar");
        assert!(lineage.contains(&"Declared in scope: global".to_string()));

        Ok(())
    }
}
