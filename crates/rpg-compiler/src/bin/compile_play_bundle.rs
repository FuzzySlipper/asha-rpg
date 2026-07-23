use std::io::{Read, Write};

fn main() {
    let mut source = Vec::new();
    std::io::stdin()
        .read_to_end(&mut source)
        .expect("read prepared PlayBundle from stdin");
    let output = match rpg_compiler::compile_prepared_play_bundle_json(&source) {
        Ok(bundle) => {
            let compiled_actions = bundle.rules().actions().collect::<Vec<_>>();
            serde_json::json!({
                "ok": true,
                "artifact": bundle.artifact(),
                "compiledActions": compiled_actions,
                "diagnostics": [],
            })
        }
        Err(failure) => serde_json::json!({
            "ok": false,
            "diagnostics": failure.diagnostics,
        }),
    };
    let encoded = serde_json::to_vec(&output).expect("encode compilation result");
    std::io::stdout()
        .write_all(&encoded)
        .expect("write compilation result");
}
