use std::io::{self, Read};

use rpg_compiler::compile_normalized_rpg_json;

fn main() {
    let mut source = Vec::new();
    io::stdin()
        .read_to_end(&mut source)
        .expect("normalized RPG IR is readable from stdin");
    let compiled = compile_normalized_rpg_json(&source).unwrap_or_else(|failure| {
        for diagnostic in failure.diagnostics {
            eprintln!(
                "{} {}: {}",
                diagnostic.code, diagnostic.path, diagnostic.message
            );
        }
        std::process::exit(1);
    });
    println!(
        "accepted {}@{} actions={}",
        compiled.package_id(),
        compiled.package_version(),
        compiled.action_ids().count()
    );
}
