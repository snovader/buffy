//! Prints each benchmark's fastest sample, from the results that the last
//! `cargo bench` left in Criterion's directory, with its ratio to the first
//! codec's, buffy's, and the median sample. The other languages' comparisons
//! report the fastest too, since load from other processes only slows samples
//! down.
//!
//! `--markdown` prints a table of the fastest samples instead, as README.md
//! shows them. An argument other than that is Criterion's directory, which is
//! `$CRITERION_HOME`, or `criterion` in `$CARGO_TARGET_DIR` or the
//! workspace's `target` by default.

use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::Deserialize;

/// One benchmark's results
struct Benchmark {
    group: String,
    codec: String,
    /// The fastest sample's time per call, in nanoseconds
    fastest: f64,
    /// The median sample's time per call, in nanoseconds
    median: f64,
    /// When its samples were written, which orders the benchmarks as they ran
    modified: SystemTime,
}

/// The benchmarks of one group, in the order they ran
struct Group {
    name: String,
    benchmarks: Vec<Benchmark>,
}

/// The part of Criterion's `benchmark.json` that names a benchmark
#[derive(Deserialize)]
struct BenchmarkId {
    group_id: String,
    function_id: Option<String>,
}

/// Criterion's `sample.json`: each sample's calls and total nanoseconds
#[derive(Deserialize)]
struct Samples {
    iters: Vec<f64>,
    times: Vec<f64>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut markdown = false;
    let mut directory = None;
    for argument in env::args().skip(1) {
        if argument == "--markdown" {
            markdown = true;
        } else {
            directory = Some(PathBuf::from(argument));
        }
    }

    let directory = directory.unwrap_or_else(criterion_directory);
    let groups = read_groups(&directory)?;
    if groups.is_empty() {
        return Err(format!(
            "No results in {}; run `cargo bench` first",
            directory.display()
        )
        .into());
    }

    if markdown {
        print_table(&groups);
    } else {
        print_groups(&groups);
    }

    Ok(())
}

/// Criterion's directory, as Criterion finds it
fn criterion_directory() -> PathBuf {
    if let Some(home) = env::var_os("CRITERION_HOME") {
        return PathBuf::from(home);
    }

    env::var_os("CARGO_TARGET_DIR")
        .map_or_else(
            || Path::new(env!("CARGO_MANIFEST_DIR")).join("../target"),
            PathBuf::from,
        )
        .join("criterion")
}

/// Reads every benchmark's last results, grouped, in the order they ran
fn read_groups(directory: &Path) -> Result<Vec<Group>, Box<dyn Error>> {
    let mut benchmarks = Vec::new();
    for group in fs::read_dir(directory)? {
        for function in fs::read_dir(group?.path())? {
            let results = function?.path().join("new");
            if results.join("sample.json").is_file() {
                benchmarks.push(read_benchmark(&results)?);
            }
        }
    }

    benchmarks.sort_by_key(|benchmark| benchmark.modified);

    let mut groups: Vec<Group> = Vec::new();
    let mut indices = HashMap::new();
    for benchmark in benchmarks {
        let index = *indices.entry(benchmark.group.clone()).or_insert_with(|| {
            groups.push(Group {
                name: benchmark.group.clone(),
                benchmarks: Vec::new(),
            });
            groups.len() - 1
        });

        groups[index].benchmarks.push(benchmark);
    }

    Ok(groups)
}

fn read_benchmark(results: &Path) -> Result<Benchmark, Box<dyn Error>> {
    let id: BenchmarkId = serde_json::from_slice(&fs::read(results.join("benchmark.json"))?)?;

    let path = results.join("sample.json");
    let samples: Samples = serde_json::from_slice(&fs::read(&path)?)?;
    let mut per_call: Vec<f64> = samples
        .times
        .iter()
        .zip(&samples.iters)
        .map(|(time, calls)| time / calls)
        .collect();
    per_call.sort_by(f64::total_cmp);

    let (Some(&fastest), Some(&median)) = (per_call.first(), per_call.get(per_call.len() / 2))
    else {
        return Err(format!("{} has no samples", path.display()).into());
    };

    Ok(Benchmark {
        group: id.group_id,
        codec: id.function_id.unwrap_or_default(),
        fastest,
        median,
        modified: fs::metadata(&path)?.modified()?,
    })
}

fn print_groups(groups: &[Group]) {
    println!("{:28} {:>12} {:>12}", "", "fastest", "median");

    for group in groups {
        println!("\n{}", group.name);

        let reference = group.benchmarks[0].fastest;
        for benchmark in &group.benchmarks {
            println!(
                "  {:26} {:>9} µs {:>9} µs {:>8}",
                benchmark.codec,
                microseconds(benchmark.fastest),
                microseconds(benchmark.median),
                ratio(benchmark.fastest / reference),
            );
        }
    }

    print_spread(groups);
}

/// Summarizes how far the medians lie above the fastest samples, which load
/// from other processes widens
fn print_spread(groups: &[Group]) {
    let spreads: Vec<f64> = groups
        .iter()
        .flat_map(|group| &group.benchmarks)
        .map(|benchmark| benchmark.median / benchmark.fastest - 1.0)
        .collect();
    let within = spreads.iter().filter(|&&spread| spread <= 0.05).count();
    let widest = spreads.iter().copied().fold(0.0, f64::max);

    println!(
        "\nThe median was within 5% of the fastest sample for {within} of {} benchmarks, and at most {:.0}% above it.",
        spreads.len(),
        widest * 100.0
    );
}

/// Prints a Markdown table of the fastest samples, with a column per codec in
/// the order they ran, and each one's ratio to the first's
fn print_table(groups: &[Group]) {
    let mut codecs: Vec<&str> = Vec::new();
    for benchmark in groups.iter().flat_map(|group| &group.benchmarks) {
        if !codecs.contains(&benchmark.codec.as_str()) {
            codecs.push(&benchmark.codec);
        }
    }

    println!("| Benchmark | {} |", codecs.join(" | "));
    println!("| --- |{}", " ---: |".repeat(codecs.len()));

    for group in groups {
        let cells: Vec<String> = codecs
            .iter()
            .map(|&codec| table_cell(group, codec))
            .collect();

        println!("| {} | {} |", group.name, cells.join(" | "));
    }
}

/// The codec's fastest sample in the group, with its ratio to the first
/// codec's, or nothing if the group has no benchmark of the codec
fn table_cell(group: &Group, codec: &str) -> String {
    let reference = &group.benchmarks[0];
    let Some(benchmark) = group.benchmarks.iter().find(|b| b.codec == codec) else {
        return String::new();
    };

    let time = microseconds(benchmark.fastest);
    if benchmark.codec == reference.codec {
        time
    } else {
        format!("{time} ({})", ratio(benchmark.fastest / reference.fastest))
    }
}

/// Formats nanoseconds as microseconds: three significant digits below 1 µs,
/// and three or one decimals above, as the other languages' tables show them
fn microseconds(nanoseconds: f64) -> String {
    let microseconds = nanoseconds / 1000.0;
    let decimals = if microseconds >= 10.0 {
        1
    } else if microseconds >= 1.0 {
        3
    } else {
        significant_decimals(microseconds, 3)
    };

    format!("{microseconds:.decimals$}")
}

/// Formats a ratio with two decimals, or three significant digits beyond them
fn ratio(ratio: f64) -> String {
    let decimals = if ratio >= 100.0 {
        0
    } else if ratio >= 10.0 {
        1
    } else if ratio >= 0.1 {
        2
    } else {
        significant_decimals(ratio, 2)
    };

    format!("{ratio:.decimals$}×")
}

/// The decimals that show `digits` significant digits of `value`, below 1
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn significant_decimals(value: f64, digits: usize) -> usize {
    if value <= 0.0 {
        return digits;
    }

    // -log10 of a value in (0, 1) is positive and small
    let leading_zeros = (-value.log10()).floor() as usize;
    leading_zeros + digits
}
