fn main() {
    // `embed_migrations!` bakes the migrations/ directory into the binary at
    // compile time. Cargo doesn't know the macro depends on those files, so
    // tell it to rebuild when a migration is added or edited.
    println!("cargo:rerun-if-changed=migrations");
}
