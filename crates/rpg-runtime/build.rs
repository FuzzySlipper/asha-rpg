use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(
        env::var("CARGO_MANIFEST_DIR").expect("Cargo supplies the manifest directory"),
    );
    let lock_path = manifest_dir.join("../../Cargo.lock");
    println!("cargo:rerun-if-changed={}", lock_path.display());

    let lock = fs::read_to_string(&lock_path).expect("workspace Cargo.lock is readable");
    let revision = package_git_revision(&lock, "asha-gameplay-module-sdk")
        .expect("the governed ASHA facade is resolved from Git");
    println!("cargo:rustc-env=ASHA_RPG_GOVERNED_ASHA_REVISION={revision}");
}

fn package_git_revision<'a>(lock: &'a str, package_name: &str) -> Option<&'a str> {
    let package_marker = format!("name = \"{package_name}\"");
    let package = lock
        .split("[[package]]")
        .find(|entry| entry.contains(&package_marker))?;
    let source = package
        .lines()
        .find_map(|line| line.trim().strip_prefix("source = \"git+"))?
        .strip_suffix('"')?;
    source.rsplit_once('#').map(|(_, revision)| revision)
}

#[cfg(test)]
mod tests {
    use super::package_git_revision;

    #[test]
    fn extracts_the_exact_resolved_git_revision() {
        let lock = r#"
[[package]]
name = "asha-gameplay-module-sdk"
version = "0.1.0"
source = "git+https://example.invalid/asha.git?rev=abc#0123456789abcdef"
"#;
        assert_eq!(
            package_git_revision(lock, "asha-gameplay-module-sdk"),
            Some("0123456789abcdef")
        );
    }
}
