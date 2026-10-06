Vendored from crates.io proc-macro-error2 2.0.1 (MIT OR Apache-2.0).
SeaORM macros require this transitive dependency. Rust 1.97 reports E0365
because its exported helper re-exports a private extern crate.
The only source change makes `extern crate proc_macro` public, as recommended
by the compiler. Test targets are omitted from the dependency manifest.
Remove this patch when the upstream dependency provides a compatible release.
