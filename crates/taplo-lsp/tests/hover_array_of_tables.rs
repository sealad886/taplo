// Diagnostic test: trace the hover code path for keys inside [[array_of_tables]]
//
// This reproduces the scenario where:
//   - hover on VALUE works (shows description)
//   - hover on KEY does not work (shows nothing)

use taplo::{
    dom::{node::Key, KeyOrIndex, Keys, Node},
    parser,
    rowan::TextSize,
    syntax::SyntaxKind,
};

// Re-use the query module from taplo-lsp
use taplo_lsp::query::{lookup_keys, Query};

const TOML_DOC: &str = r#"[training]
batch_size = 4

[[training.curriculum]]
name = "phase_one"
weight = 1.0

[[training.curriculum]]
name = "phase_two"
weight = 2.0
"#;

fn parse_doc() -> Node {
    let parsed = parser::parse(TOML_DOC);
    assert!(parsed.errors.is_empty(), "parse errors: {:?}", parsed.errors);
    parsed.into_dom()
}

#[test]
fn test_dom_keys_for_name_key() {
    let root = parse_doc();

    println!("\n=== flat_iter output ===");
    for (keys, node) in root.flat_iter() {
        let keys_debug: Vec<String> = keys.iter().map(|k| format!("{k:?}")).collect();
        let text_ranges: Vec<_> = node.text_ranges(false).collect();
        println!(
            "  keys=[{}]  node_kind={:?}  ranges={:?}",
            keys_debug.join(", "),
            std::mem::discriminant(&node),
            text_ranges,
        );
    }
}

#[test]
fn test_query_at_key_name() {
    let root = parse_doc();

    // Find the byte offset of the first "name" key in "name = \"phase_one\""
    let name_offset = TOML_DOC
        .find("name")
        .expect("could not find 'name' in TOML doc");
    println!("\n=== Querying at 'name' key offset={name_offset} ===");
    println!("Context: {:?}", &TOML_DOC[name_offset..name_offset + 20]);

    let query = Query::at(&root, TextSize::from(name_offset as u32));

    // Check before
    if let Some(before) = &query.before {
        println!(
            "  before.syntax: kind={:?} text={:?}",
            before.syntax.kind(),
            before.syntax.text()
        );
        if let Some((keys, _node)) = &before.dom_node {
            let keys_debug: Vec<String> = keys.iter().map(|k| format!("{k:?}")).collect();
            println!("  before.dom_node keys: [{}]", keys_debug.join(", "));
        } else {
            println!("  before.dom_node: None");
        }
    } else {
        println!("  before: None");
    }

    // Check after
    if let Some(after) = &query.after {
        println!(
            "  after.syntax: kind={:?} text={:?}",
            after.syntax.kind(),
            after.syntax.text()
        );
        if let Some((keys, _node)) = &after.dom_node {
            let keys_debug: Vec<String> = keys.iter().map(|k| format!("{k:?}")).collect();
            println!("  after.dom_node keys: [{}]", keys_debug.join(", "));
        } else {
            println!("  after.dom_node: None");
        }
    } else {
        println!("  after: None");
    }

    // Check header_key
    let header_key = query.header_key();
    println!("  header_key: {:?}", header_key.as_ref().map(|h| h.to_string()));

    // Determine position_info (same logic as hover handler)
    let position_info = query
        .before
        .clone()
        .filter(|p| {
            p.syntax.kind() == SyntaxKind::IDENT
                || matches!(
                    p.syntax.kind(),
                    SyntaxKind::BOOL
                        | SyntaxKind::STRING
                        | SyntaxKind::INTEGER
                        | SyntaxKind::FLOAT
                )
        })
        .or_else(|| {
            query.after.clone().filter(|p| {
                p.syntax.kind() == SyntaxKind::IDENT
                    || matches!(
                        p.syntax.kind(),
                        SyntaxKind::BOOL
                            | SyntaxKind::STRING
                            | SyntaxKind::INTEGER
                            | SyntaxKind::FLOAT
                    )
            })
        });

    if let Some(pi) = &position_info {
        println!(
            "\n  position_info: kind={:?} text={:?}",
            pi.syntax.kind(),
            pi.syntax.text()
        );

        if let Some((keys, _node)) = &pi.dom_node {
            let keys_debug: Vec<String> = keys.iter().map(|k| format!("{k:?}")).collect();
            println!("  pi.dom_node keys: [{}]", keys_debug.join(", "));

            let mut keys = keys.clone();

            // Check header_key adjustment
            if let Some(ref header_key) = header_key {
                let key_idx = header_key
                    .descendants_with_tokens()
                    .filter(|t| t.kind() == SyntaxKind::IDENT)
                    .position(|t| t.as_token().unwrap() == &pi.syntax);
                println!("  header_key key_idx: {:?}", key_idx);
                if let Some(key_idx) = key_idx {
                    keys = lookup_keys(
                        root.clone(),
                        &Keys::new(keys.into_iter().take(key_idx + 1)),
                    );
                    println!(
                        "  keys after header_key adjustment: [{}]",
                        keys.iter()
                            .map(|k| format!("{k:?}"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
            }

            // Check dom.path
            let node = root.path(&keys);
            println!("  dom.path(&keys) exists: {}", node.is_some());

            if pi.syntax.kind() == SyntaxKind::IDENT {
                println!("\n  === IDENT path (key hover) ===");
                let lookup = lookup_keys(root.clone(), &keys);
                let lookup_debug: Vec<String> =
                    lookup.iter().map(|k| format!("{k:?}")).collect();
                println!("  after lookup_keys: [{}]", lookup_debug.join(", "));

                // Strip trailing Index
                let mut stripped = lookup;
                while let Some(KeyOrIndex::Index(_)) = stripped.iter().last() {
                    stripped = stripped.skip_right(1);
                }
                let stripped_debug: Vec<String> =
                    stripped.iter().map(|k| format!("{k:?}")).collect();
                println!("  after stripping trailing Index: [{}]", stripped_debug.join(", "));
                println!("  -> These are the keys used for schema lookup in IDENT path");
            }
        } else {
            println!("  pi.dom_node: None  ← THIS WOULD CAUSE EARLY RETURN");
        }
    } else {
        println!("\n  position_info: None  ← THIS WOULD CAUSE EARLY RETURN");
    }
}

#[test]
fn test_query_at_value() {
    let root = parse_doc();

    // Find the byte offset of "phase_one" (the value)
    let value_offset = TOML_DOC
        .find("\"phase_one\"")
        .expect("could not find value in TOML doc");
    // Move inside the quotes to hit STRING kind
    let value_offset = value_offset + 1; // skip opening quote
    println!("\n=== Querying at value offset={value_offset} ===");
    println!("Context: {:?}", &TOML_DOC[value_offset..value_offset + 10]);

    let query = Query::at(&root, TextSize::from(value_offset as u32));

    if let Some(after) = &query.after {
        println!(
            "  after.syntax: kind={:?} text={:?}",
            after.syntax.kind(),
            after.syntax.text()
        );
        if let Some((keys, _node)) = &after.dom_node {
            let keys_debug: Vec<String> = keys.iter().map(|k| format!("{k:?}")).collect();
            println!("  after.dom_node keys: [{}]", keys_debug.join(", "));
            println!("  -> These are the keys used for schema lookup in is_primitive path");
        }
    }
}

#[test]
fn test_query_at_header_curriculum() {
    let root = parse_doc();

    // Find the byte offset of "curriculum" in [[training.curriculum]]
    let curriculum_offset = TOML_DOC
        .find("curriculum")
        .expect("could not find 'curriculum' in TOML doc");
    println!("\n=== Querying at header 'curriculum' offset={curriculum_offset} ===");
    println!(
        "Context: {:?}",
        &TOML_DOC[curriculum_offset..curriculum_offset + 15]
    );

    let query = Query::at(&root, TextSize::from(curriculum_offset as u32));

    if let Some(before) = &query.before {
        println!(
            "  before.syntax: kind={:?} text={:?}",
            before.syntax.kind(),
            before.syntax.text()
        );
    }
    if let Some(after) = &query.after {
        println!(
            "  after.syntax: kind={:?} text={:?}",
            after.syntax.kind(),
            after.syntax.text()
        );
        if let Some((keys, _node)) = &after.dom_node {
            let keys_debug: Vec<String> = keys.iter().map(|k| format!("{k:?}")).collect();
            println!("  after.dom_node keys: [{}]", keys_debug.join(", "));
        }
    }

    let header_key = query.header_key();
    println!("  header_key: {:?}", header_key.as_ref().map(|h| h.to_string()));
}

#[test]
fn test_lookup_keys_with_index() {
    let root = parse_doc();

    // Simulate the keys that position_info_at would produce for "name" inside
    // the first [[training.curriculum]] entry
    println!("\n=== lookup_keys traces ===");

    // Path WITHOUT index (as if we just had the key names):
    let keys_without_idx = Keys::new(
        [
            KeyOrIndex::Key(Key::from("training")),
            KeyOrIndex::Key(Key::from("curriculum")),
        ]
        .into_iter(),
    );
    let result = lookup_keys(root.clone(), &keys_without_idx);
    println!(
        "  lookup_keys([training, curriculum]) = [{}]",
        result
            .iter()
            .map(|k| format!("{k:?}"))
            .collect::<Vec<_>>()
            .join(", ")
    );

    // Path WITH index (as position_info_at provides for array-of-tables):
    let keys_with_idx = Keys::new(
        [
            KeyOrIndex::Key(Key::from("training")),
            KeyOrIndex::Key(Key::from("curriculum")),
            KeyOrIndex::Index(0),
            KeyOrIndex::Key(Key::from("name")),
        ]
        .into_iter(),
    );
    let result = lookup_keys(root.clone(), &keys_with_idx);
    println!(
        "  lookup_keys([training, curriculum, Index(0), name]) = [{}]",
        result
            .iter()
            .map(|k| format!("{k:?}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
}
