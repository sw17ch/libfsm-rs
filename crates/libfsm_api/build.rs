use std::env;
use std::path::{Path, PathBuf};

fn build_fsm() {
    let lexer_c = PathBuf::from("vendor/libfsm/src/libfsm/lexer.c");

    // There are a few special files that need their own construction.
    let lexer_o = cc::Build::new()
        .file(&lexer_c)
        .flag("--std=c99")
        .flag("-pedantic")
        .define("LX_HEADER", "\"lexer.h\"")
        .warnings(false)
        .warnings_into_errors(false)
        .compile_intermediates();

    let paths = &[
        "vendor/libfsm/src/adt/*.c",
        "vendor/libfsm/src/print/*.c",
        "vendor/libfsm/src/libfsm/**/*.c",
    ];
    let special = &[lexer_c.as_path()];
    let cfiles = paths
        .map(|p| {
            glob::glob(p)
                .expect("failed to read glob pattern")
                .map(|e| e.expect("failed to get glob entry"))
        })
        .into_iter()
        .flatten()
        .filter_map(|e| special.iter().all(|s| &e != s).then_some(e));

    let cfiles = cfiles.collect::<Vec<_>>();

    cc::Build::new()
        .include("vendor/libfsm/include/")
        .include("vendor/libfsm/src/")
        .include("vendor/libfsm/src/libfsm/")
        .flag("--std=c99")
        .flag("-pedantic")
        .files(cfiles)
        .objects(lexer_o)
        .compile("fsm");
}

fn build_re_dialect(which: &str) -> Vec<PathBuf> {
    let base = "vendor/libfsm/src/libre/dialect";
    let lexer = format!("{base}/{which}/lexer.c");
    let parser = format!("{base}/{which}/parser.c");
    let dialect = format!("{base}/{which}/re_dialect_{which}.c");

    let mut ret = vec![];

    ret.extend(
        cc::Build::new()
            .file(&lexer)
            .flag("--std=c99")
            .flag("-pedantic")
            .define("LX_HEADER", "\"lexer.h\"")
            .include("vendor/libfsm/include/")
            .include("vendor/libfsm/src/")
            .warnings(false)
            .warnings_into_errors(false)
            .compile_intermediates(),
    );
    ret.extend({
        let mut build = cc::Build::new();
        build
            .file(&parser)
            .flag("--std=c99")
            .flag("-pedantic")
            .define("DIALECT", which)
            .include("vendor/libfsm/include/")
            .include("vendor/libfsm/src/")
            .warnings(false)
            .warnings_into_errors(false);
        if which == "pcre" {
            build.define("PCRE_DIALECT", "1");
        }
        build.compile_intermediates()
    });
    ret.extend(
        cc::Build::new()
            .file(&dialect)
            .flag("--std=c99")
            .flag("-pedantic")
            .include("vendor/libfsm/include/")
            .include("vendor/libfsm/src/")
            .warnings(false)
            .warnings_into_errors(false)
            .compile_intermediates(),
    );

    ret
}

fn build_re() {
    let mut objs = vec![];

    objs.extend(build_re_dialect("glob"));
    objs.extend(build_re_dialect("like"));
    objs.extend(build_re_dialect("literal"));
    objs.extend(build_re_dialect("native"));
    objs.extend(build_re_dialect("pcre"));
    objs.extend(build_re_dialect("sql"));

    let paths = &[
        "vendor/libfsm/src/libre/*.c",
        "vendor/libfsm/src/libre/class/*.c",
        "vendor/libfsm/src/libre/print/*.c",
    ];
    let special: &[&Path] = &[];
    let cfiles = paths
        .map(|p| {
            glob::glob(p)
                .expect("failed to read glob pattern")
                .map(|e| e.expect("failed to get glob entry"))
        })
        .into_iter()
        .flatten()
        .filter_map(|e| special.iter().all(|s| &e != s).then_some(e));

    let cfiles = cfiles.collect::<Vec<_>>();

    cc::Build::new()
        .include("vendor/libfsm/include/")
        .include("vendor/libfsm/src/")
        .include("vendor/libfsm/src/libre/")
        .flag("--std=c99")
        .flag("-pedantic")
        .define("LF_HEADER", "\"class.h\"")
        .define("LX_HEADER", "\"lexer.h\"")
        .files(cfiles)
        .objects(objs)
        .compile("re");
}

fn main() {
    build_fsm();
    build_re();

    println!("cargo::rerun-if-changed=wrapper.h");

    // The bindgen::Builder is the main entry point
    // to bindgen, and lets you build up options for
    // the resulting bindings.
    let bindings = bindgen::Builder::default()
        // The input header we would like to generate
        // bindings for.
        .header("wrapper.h")
        // Tell cargo to invalidate the built crate whenever any of the
        // included header files changed.
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        // Generate default instances for types.
        .derive_default(true)
        .allowlist_function("re_(comp|strerror)")
        .allowlist_function("fsm_(determinise|free|print)")
        // Finish the builder and generate the bindings.
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
