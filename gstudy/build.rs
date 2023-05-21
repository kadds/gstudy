use vergen::EmitBuilder;

fn main() {
    EmitBuilder::builder()
        .all_build()
        .all_cargo()
        .all_git()
        .emit()
        .unwrap();
}
