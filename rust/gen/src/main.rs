//! `tafla-gen <target> <schema>... [--out <dir>] [--runtime <module>]
//! [--package <package>]`

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use tafla_gen::{Diagnostic, Options, Target, compile};

fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprint!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn run(arguments: Vec<String>) -> Result<(), String> {
    let mut arguments = arguments.into_iter();
    let target = arguments
        .next()
        .and_then(|name| Target::from_name(&name))
        .ok_or_else(usage)?;

    let mut schemas = Vec::new();
    let mut out = PathBuf::from(".");
    let mut options = Options::default();
    while let Some(argument) = arguments.next() {
        let mut value = || {
            arguments
                .next()
                .ok_or(format!("{argument} needs a value\n"))
        };
        match argument.as_str() {
            "--out" => out = PathBuf::from(value()?),
            "--runtime" => options.runtime = Some(value()?),
            "--package" => options.package = Some(value()?),
            _ => schemas.push(argument),
        }
    }
    if schemas.is_empty() {
        return Err(usage());
    }

    let mut errors = Vec::new();
    for schema in schemas {
        let source =
            std::fs::read_to_string(&schema).map_err(|error| format!("{schema}: {error}\n"))?;
        match compile(&schema, &source, target, &options) {
            Ok(files) => {
                for file in files {
                    write(&out.join(&file.path), &file.contents)?;
                }
            }
            Err(diagnostics) => errors.push(Diagnostic::render_all(&diagnostics, &schema, &source)),
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("\n"))
    }
}

fn usage() -> String {
    let targets: Vec<_> = Target::names().collect();
    format!(
        "usage: tafla-gen <{}> <schema>... [--out <dir>] [--runtime <module>] [--package <package>]\n",
        targets.join("|")
    )
}

fn write(path: &Path, contents: &str) -> Result<(), String> {
    if let Some(directory) = path.parent() {
        std::fs::create_dir_all(directory)
            .map_err(|error| format!("{}: {error}\n", directory.display()))?;
    }
    std::fs::write(path, contents).map_err(|error| format!("{}: {error}\n", path.display()))
}
