use std::collections::HashMap;
use std::error::Error;
use std::path::Path;
use tree_sitter_graph::graph::Value;
use tree_sitter_graph::graph::{Graph, GraphNodeRef};
use tree_sitter_graph::Identifier;

#[derive(Debug, Clone)]
struct Declaration {
    pub name: String,
    pub location: Location,
    pub scope: String,
    pub references: Vec<Reference>,
}

#[derive(Debug, Clone)]
struct Reference {
    pub location: Location,
    pub context: String,
}

#[derive(Debug, Clone)]
struct Location {
    pub line: usize,
    pub column: usize,
    pub length: usize,
}

pub struct DataLineageTracker {
    declarations: HashMap<String, Declaration>,
    source_code: String,
}

impl DataLineageTracker {
    pub fn new() -> Self {
        Self {
            declarations: HashMap::new(),
            source_code: String::new(),
        }
    }

    pub fn analyze_file<P: AsRef<Path>>(&mut self, path: P) -> Result<(), Box<dyn Error>> {
        self.source_code = std::fs::read_to_string(path)?;

        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(tree_sitter_javascript::language())
            .map_err(|e| Box::new(e) as Box<dyn Error>)?;

        let tree = parser
            .parse(&self.source_code, None)
            .ok_or("Failed to parse source code")?;

        let mut graph = Graph::new();
        let root_node = graph.add_graph_node();
        let root_node_mut = &mut graph[root_node];
        root_node_mut
            .attributes
            .add(Identifier::from("type"), Value::from("root"))
            .ok();

        // Start traversal with the root node
        self.traverse_tree(tree.root_node(), &mut graph, &root_node)?;

        println!("Generated Graph:");
        println!("{}", graph.pretty_print());

        Ok(())
    }

    fn traverse_tree<'a>(
        &mut self,
        node: tree_sitter::Node<'a>,
        graph: &mut Graph<'a>,
        parent_ref: &GraphNodeRef,
    ) -> Result<(), Box<dyn Error>> {
        match node.kind() {
            "variable_declarator" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    if let Ok(name) = name_node.utf8_text(self.source_code.as_bytes()) {
                        let scope = self.determine_scope(&node);
                        
                        // Store the declaration in the HashMap
                        let start_pos = name_node.start_position();
                        self.declarations.insert(
                            name.to_string(),
                            Declaration {
                                name: name.to_string(),
                                location: Location {
                                    line: start_pos.row + 1,
                                    column: start_pos.column + 1,
                                    length: name_node.end_byte() - name_node.start_byte(),
                                },
                                scope: scope.clone(),
                                references: Vec::new(),
                            },
                        );

                        // Create graph nodes as before
                        let _syntax_ref = graph.add_syntax_node(name_node);
                        let decl_node = graph.add_graph_node();

                        let parent_node = &mut graph[*parent_ref];
                        if let Err(_) = parent_node.add_edge(decl_node) {
                            // Edge already exists, we can continue
                        }

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
                    }
                }
            }
            "identifier" => {
                if let Ok(name) = node.utf8_text(self.source_code.as_bytes()) {
                    let scope = self.determine_scope(&node);
                    let start_pos = node.start_position();

                    // Update references in the HashMap
                    if let Some(decl) = self.declarations.get_mut(&name.to_string()) {
                        decl.references.push(Reference {
                            location: Location {
                                line: start_pos.row + 1,
                                column: start_pos.column + 1,
                                length: node.end_byte() - node.start_byte(),
                            },
                            context: scope.clone(),
                        });
                    }

                    // Create graph nodes as before
                    let _syntax_ref = graph.add_syntax_node(node);
                    let ref_node = graph.add_graph_node();

                    let parent_node = &mut graph[*parent_ref];
                    if let Err(_) = parent_node.add_edge(ref_node) {
                        // Edge already exists, we can continue
                    }

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
                }
            }
            _ => {}
        }

        // Traverse children using simple iteration
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.traverse_tree(child, graph, parent_ref)?;
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
        if let Some(decl) = self.declarations.get(variable_name) {
            lineage.push(format!("Declared in scope: {}", decl.scope));
            for ref_ in &decl.references {
                lineage.push(format!("Referenced in scope: {}", ref_.context));
            }
        }
        lineage
    }

    pub fn print_lineage(&self) {
        println!("Variable Declarations and References:");
        println!("===================================");
        for (name, declaration) in &self.declarations {
            println!("\nVariable: {}", name);
            println!(
                " Declared at line {}, column {}",
                declaration.location.line, declaration.location.column
            );
            println!(" Scope: {}", declaration.scope);
            if declaration.references.is_empty() {
                println!(" No references found");
            } else {
                println!(" References:");
                for reference in &declaration.references {
                    println!(
                        " - At line {}, column {} (in scope: {})",
                        reference.location.line, reference.location.column, reference.context
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
