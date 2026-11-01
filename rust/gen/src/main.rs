//! `buffy-gen <target> <schema>... [--out <dir>] [--runtime <module>]
//! [--package <package>]`

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use buffy_gen::{Diagnostic, GeneratedFile, JavaPackage, Options, Target, compile};

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
            "--package" => {
                let package = value()?.parse::<JavaPackage>();
                options.package = Some(package.map_err(|error| format!("{error}\n"))?);
            }
            // A schema given twice, such as by overlapping globs, is
            // generated once
            _ if schemas.contains(&argument) => {}
            _ => schemas.push(argument),
        }
    }
    if schemas.is_empty() {
        return Err(usage());
    }

    // Every schema is compiled before any is written, so that schemas whose
    // files would overwrite each other's are found first
    let mut compiled = Vec::new();
    let mut errors = Vec::new();
    for schema in schemas {
        let source =
            std::fs::read_to_string(&schema).map_err(|error| format!("{schema}: {error}\n"))?;
        match compile(&schema, &source, target, &options) {
            Ok(files) => compiled.push((schema, files)),
            Err(diagnostics) => errors.push(Diagnostic::render_all(&diagnostics, &schema, &source)),
        }
    }

    let collisions = collisions(&compiled);
    // Neither schema of a collision is written, since either would hide the
    // other's file
    let colliding: HashSet<usize> = collisions
        .iter()
        .flat_map(|collision| collision.schemas)
        .collect();
    for (index, (_, files)) in compiled.iter().enumerate() {
        if colliding.contains(&index) {
            continue;
        }
        for file in files {
            write(&out.join(&file.path), &file.contents)?;
        }
    }
    errors.extend(
        collisions
            .iter()
            .map(|collision| collision.describe(&compiled)),
    );

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("\n"))
    }
}

fn usage() -> String {
    let targets: Vec<_> = Target::names().collect();
    format!(
        "usage: buffy-gen <{}> <schema>... [--out <dir>] [--runtime <module>] [--package <package>]\n",
        targets.join("|")
    )
}

/// Two generated files that would be one on disk: their paths are the same,
/// or differ only in case, which macOS and Windows ignore by default
struct Collision<'a> {
    /// The indices of the files' schemas, in the order they were given,
    /// which are the same for classes of one Java schema
    schemas: [usize; 2],
    paths: [&'a str; 2],
}

impl Collision<'_> {
    fn describe(&self, compiled: &[(String, Vec<GeneratedFile>)]) -> String {
        let [first, second] = self.schemas.map(|index| compiled[index].0.as_str());
        match self.paths {
            [path, other] if path == other => format!("{first} and {second} both write {path}\n"),
            [path, other] => format!(
                "{first} writes {path} and {second} writes {other}, \
                 which are one file where file names ignore case\n"
            ),
        }
    }
}

/// The generated files that would overwrite a file generated before them
fn collisions(compiled: &[(String, Vec<GeneratedFile>)]) -> Vec<Collision<'_>> {
    // Each path in lowercase, with the schema and path that take it first
    let mut taken: HashMap<String, (usize, &str)> = HashMap::new();
    let mut collisions = Vec::new();

    for (index, (_, files)) in compiled.iter().enumerate() {
        for file in files {
            match taken.entry(file.path.to_lowercase()) {
                Entry::Vacant(entry) => {
                    entry.insert((index, &file.path));
                }
                Entry::Occupied(entry) => {
                    let &(first, path) = entry.get();
                    collisions.push(Collision {
                        schemas: [first, index],
                        paths: [path, &file.path],
                    });
                }
            }
        }
    }

    collisions
}

fn write(path: &Path, contents: &str) -> Result<(), String> {
    if let Some(directory) = path.parent() {
        std::fs::create_dir_all(directory)
            .map_err(|error| format!("{}: {error}\n", directory.display()))?;
    }
    std::fs::write(path, contents).map_err(|error| format!("{}: {error}\n", path.display()))
}
