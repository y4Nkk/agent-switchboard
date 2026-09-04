use std::path::PathBuf;

fn main() {
    let output = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../website/src/generated/configuration-assembly.ts");
    let generated = asb_core::website_assembly::typescript();
    let check = std::env::args()
        .skip(1)
        .any(|argument| argument == "--check");

    if check {
        let committed = std::fs::read_to_string(&output)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", output.display()));
        if committed != generated {
            panic!(
                "{} is stale; run cargo run -p asb-core --bin generate-website-assembly",
                output.display()
            );
        }
        return;
    }

    let parent = output.parent().expect("website generated directory");
    std::fs::create_dir_all(parent)
        .unwrap_or_else(|error| panic!("cannot create {}: {error}", parent.display()));
    std::fs::write(&output, generated)
        .unwrap_or_else(|error| panic!("cannot write {}: {error}", output.display()));
}
