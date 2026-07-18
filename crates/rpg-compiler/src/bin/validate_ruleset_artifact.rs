use std::io::Read;

fn main() {
    let mut source = Vec::new();
    std::io::stdin()
        .read_to_end(&mut source)
        .expect("read compiled ruleset artifact from stdin");
    match rpg_compiler::load_compiled_ruleset_artifact_json(&source) {
        Ok(bundle) => println!(
            "accepted {} definitions={}",
            bundle.artifact().artifact_id,
            bundle.artifact().materialized_definitions.len()
        ),
        Err(failure) => {
            eprintln!("{failure}");
            for diagnostic in failure.diagnostics {
                eprintln!(
                    "{} {}: {}",
                    diagnostic.code, diagnostic.path, diagnostic.message
                );
            }
            std::process::exit(1);
        }
    }
}
