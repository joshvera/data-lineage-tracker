use std::collections::HashMap;
use std::error::Error;
use std::path::Path;
use tree_sitter_graph::graph::Graph;
use tree_sitter_graph::ParseError;

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

        // Create a new parser
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(tree_sitter_javascript::language())
            .map_err(|e| Box::new(e) as Box<dyn Error>)?;

        // Parse the source code
        let tree = parser
            .parse(&self.source_code, None)
            .ok_or("Failed to parse source code")?;

        // Create a new graph
        let mut graph = Graph::new();

        // Add the root node to the graph
        let root_node = tree.root_node();
        let _root_ref = graph.add_syntax_node(root_node);

        // Create cursor for tree traversal
        let mut cursor = root_node.walk();
        self.traverse_tree(&mut cursor)?;

        // We can use pretty_print for debugging
        println!("Generated Graph:");
        println!("{}", graph.pretty_print());

        Ok(())
    }

    fn traverse_tree(
        &mut self,
        cursor: &mut tree_sitter::TreeCursor,
    ) -> Result<(), Box<dyn Error>> {
        let node = cursor.node();

        // Process current node
        match node.kind() {
            "variable_declarator" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    if let Ok(name) = name_node.utf8_text(self.source_code.as_bytes()) {
                        // Determine scope first
                        let scope = self.determine_scope(&node);

                        // Store the declaration
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
                                scope,
                                references: Vec::new(),
                            },
                        );
                    }
                }
            }
            "identifier" => {
                if let Ok(name) = node.utf8_text(self.source_code.as_bytes()) {
                    // Determine scope before mutable borrow
                    let scope = self.determine_scope(&node);
                    let start_pos = node.start_position();

                    // Now do the mutable borrow
                    if let Some(decl) = self.declarations.get_mut(&name.to_string()) {
                        decl.references.push(Reference {
                            location: Location {
                                line: start_pos.row + 1,
                                column: start_pos.column + 1,
                                length: node.end_byte() - node.start_byte(),
                            },
                            context: scope,
                        });
                    }
                }
            }
            _ => {}
        }

        // Traverse children
        if cursor.goto_first_child() {
            loop {
                self.traverse_tree(cursor)?;
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
            cursor.goto_parent();
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
