use std::fs;
use std::path::{Path, PathBuf};

use tacit_canonical::ast::Node;
use tacit_canonical::{emit, hash_node};
use tacit_typecheck::{
    check_package, load_package, lock_package, package_test_entry_expression, PackageGraph,
};

const ASCII_IS_DIGIT: &str = "f7babbf21591eeeb64d2c990e40b6be53def9032770ae10c346c7e3132173a5a";
const ASCII_IS_UPPER_PACKAGE: &str =
    "34123b1ba54b1040d1ee8e139e9dcbb66a6c2e128c2754f1f30bada52f1971b6";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate lives under repo/crates/tacit-typecheck")
        .to_path_buf()
}

fn stdlib_package(name: &str) -> PathBuf {
    repo_root().join("stdlib").join("tacit").join(name)
}

fn copy_dir(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap();
    let mut entries: Vec<_> = fs::read_dir(src)
        .unwrap_or_else(|err| panic!("read {}: {}", src.display(), err))
        .map(|entry| entry.unwrap().path())
        .collect();
    entries.sort();

    for entry in entries {
        let name = entry.file_name().unwrap();
        if name == ".tacit" {
            continue;
        }
        let target = dst.join(name);
        if entry.is_dir() {
            copy_dir(&entry, &target);
        } else {
            fs::copy(&entry, &target)
                .unwrap_or_else(|err| panic!("copy {}: {}", entry.display(), err));
        }
    }
}

fn copy_stdlib_package(workspace: &Path, name: &str) -> PathBuf {
    let dst = workspace.join(name);
    copy_dir(&stdlib_package(name), &dst);
    dst
}

fn sym(name: &str) -> Node {
    Node::Sym { name: name.into() }
}

fn eff(atoms: &[&str]) -> Node {
    Node::EffSet {
        atoms: atoms.iter().map(|atom| (*atom).to_string()).collect(),
    }
}

fn int_to_bool_sig() -> Node {
    Node::Sig {
        type_: Box::new(Node::FnTy {
            arg: Box::new(sym("Int")),
            ret: Box::new(sym("Bool")),
            eff: Box::new(eff(&[])),
        }),
        eval_eff: Box::new(eff(&[])),
    }
}

fn bool_sig() -> Node {
    Node::Sig {
        type_: Box::new(sym("Bool")),
        eval_eff: Box::new(eff(&[])),
    }
}

fn hash(node: &Node) -> String {
    hash_node(node)
        .iter()
        .map(|byte| format!("{:02x}", byte))
        .collect()
}

fn write_unit(root: &Path, rel: &str, unit: &Node) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, emit(unit)).unwrap();
}

fn write_consumer_package(root: &Path, dep_path: &Path, import_hash: &str, byte: &str) -> String {
    let def = Node::Def {
        sig: Box::new(bool_sig()),
        body: Box::new(Node::App {
            fn_: Box::new(Node::Ref {
                hash: import_hash.into(),
            }),
            arg: Box::new(Node::Int { value: byte.into() }),
        }),
    };
    let def_hash = hash(&def);
    let unit = Node::Unit {
        imports: vec![Node::Import {
            hash: import_hash.into(),
            sig: Box::new(int_to_bool_sig()),
        }],
        exports: vec![],
        defs: vec![def],
    };
    write_unit(root, "src/tests.tac", &unit);
    fs::write(
        root.join("tacit.toml"),
        format!(
            r#"[package]
name = "stdlib-consumer"

[dependencies]
text = {{ path = "{}" }}

[[tests]]
name = "classifies_byte"
target = "blake3:{}"
"#,
            dep_path.display(),
            def_hash
        ),
    )
    .unwrap();
    def_hash
}

fn checked_copy(workspace: &Path, name: &str) -> PackageGraph {
    let root = copy_stdlib_package(workspace, name);
    let package = load_package(&root).unwrap_or_else(|diags| {
        panic!(
            "load stdlib package {name}:\n{}",
            diags
                .iter()
                .map(|diag| format!("{}: {}", diag.kind, diag.message))
                .collect::<Vec<_>>()
                .join("\n")
        )
    });
    check_package(&package).unwrap_or_else(|diags| {
        panic!(
            "check stdlib package {name}:\n{}",
            diags
                .iter()
                .map(|diag| format!("{}: {}", diag.kind, diag.message))
                .collect::<Vec<_>>()
                .join("\n")
        )
    });
    package
}

#[test]
fn source_stdlib_packages_are_ordinary_packages() {
    let workspace = tempfile::tempdir().unwrap();
    for (name, expected_exports) in [
        ("core", 2),
        ("bytes", 10),
        ("array", 13),
        ("text", 9),
        ("collections", 5),
        ("io", 10),
    ] {
        let package = checked_copy(workspace.path(), name);
        assert_eq!(
            package
                .manifest
                .package
                .as_ref()
                .and_then(|metadata| metadata.name.as_deref()),
            Some(match name {
                "core" => "tacit.core",
                "bytes" => "tacit.bytes",
                "array" => "tacit.array",
                "text" => "tacit.text",
                "collections" => "tacit.collections",
                "io" => "tacit.io",
                _ => unreachable!(),
            })
        );
        let public_exports: usize = package
            .root
            .units
            .iter()
            .map(|unit| unit.public_exports.len())
            .sum();
        assert_eq!(public_exports, expected_exports, "{name}");
    }
}

#[test]
fn source_stdlib_is_consumed_through_path_dependency_and_exact_import() {
    let workspace = tempfile::tempdir().unwrap();
    let text_root = copy_stdlib_package(workspace.path(), "text");
    let app_root = workspace.path().join("app");
    fs::create_dir_all(&app_root).unwrap();

    let target_hash = write_consumer_package(&app_root, &text_root, ASCII_IS_DIGIT, "57");
    let package = lock_package(&app_root).expect("path dependency writes lockfile");
    check_package(&package).expect("consumer checks against stdlib dependency");

    assert_eq!(package.dependencies.len(), 1);
    assert_eq!(
        package.lockfile.as_ref().unwrap().dependencies[0].alias,
        "text"
    );
    let entry = package_test_entry_expression(&package, &target_hash).expect("expand test entry");
    let entry_text = String::from_utf8(emit(&entry.expression)).unwrap();
    assert!(!entry_text.contains("(ref "), "{entry_text}");
    assert!(entry_text.contains("(int 48)"), "{entry_text}");
    assert!(entry_text.contains("(int 57)"), "{entry_text}");
}

#[test]
fn package_local_stdlib_helpers_are_not_externally_importable() {
    let workspace = tempfile::tempdir().unwrap();
    let text_root = copy_stdlib_package(workspace.path(), "text");
    let app_root = workspace.path().join("app");
    fs::create_dir_all(&app_root).unwrap();

    write_consumer_package(&app_root, &text_root, ASCII_IS_UPPER_PACKAGE, "65");
    let package = lock_package(&app_root).expect("path dependency writes lockfile");
    let diags = check_package(&package).expect_err("package export is not external API");
    assert!(
        diags.iter().any(|diag| diag.kind == "visibility-violation"),
        "{:?}",
        diags.iter().map(|diag| &diag.kind).collect::<Vec<_>>()
    );
}
