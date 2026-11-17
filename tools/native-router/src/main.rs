//! `native-router` CLI: build a persisted car graph from a local PBF, or route
//! from that cache, and print the route as JSON. Diagnostics go to stderr so
//! stdout stays machine-readable.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use native_router::{load_pbf, route, Graph, Profile};

const USAGE: &str = "\
Usage:
  native-router build --osm <file.osm.pbf> --out <graph.bin> [--profile car]
  native-router route --graph <graph.bin> --from <lat,lon> --to <lat,lon> [--profile car]
  native-router route --osm <file.osm.pbf> --from <lat,lon> --to <lat,lon> [--profile car]
  native-router --help

`build` parses the PBF once and writes a checksummed graph cache. `route
--graph` loads that cache instead of re-parsing the PBF; `--osm` is the
one-shot fallback. Routing is local: the PBF or graph file is the only input.
No HTTP or online service is used.";

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}");
            eprintln!();
            eprintln!("{USAGE}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        None | Some("--help" | "-h" | "help") => {
            println!("{USAGE}");
            Ok(())
        }
        Some("build") => run_build(&args[1..]),
        Some("route") => run_route(&args[1..]),
        Some(other) => Err(format!("unknown command `{other}`")),
    }
}

fn run_build(args: &[String]) -> Result<(), String> {
    let mut osm: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut profile = Profile::Car;

    let mut index = 0;
    while index < args.len() {
        let flag = args[index].as_str();
        match flag {
            "--osm" => {
                osm = Some(PathBuf::from(flag_value(args, index, flag)?));
                index += 2;
            }
            "--out" => {
                out = Some(PathBuf::from(flag_value(args, index, flag)?));
                index += 2;
            }
            "--profile" => {
                profile = parse_profile(flag_value(args, index, flag)?)?;
                index += 2;
            }
            "--help" | "-h" => {
                println!("{USAGE}");
                return Ok(());
            }
            other => return Err(format!("unknown argument `{other}`")),
        }
    }
    let _ = profile;

    let osm = osm.ok_or("`--osm` is required for `build`")?;
    let out = out.ok_or("`--out` is required for `build`")?;

    let build_started = Instant::now();
    let graph = load_pbf(&osm).map_err(|err| err.to_string())?;
    let build_elapsed = build_started.elapsed();
    print_graph_stats(&graph, build_elapsed, "built from PBF");

    let save_started = Instant::now();
    graph.save(&out).map_err(|err| err.to_string())?;
    let save_elapsed = save_started.elapsed();
    let bytes = fs::metadata(&out).map_err(|err| err.to_string())?.len();
    eprintln!(
        "saved {} ({} bytes, {}) in {:.2} s",
        out.display(),
        bytes,
        format_bytes(bytes),
        save_elapsed.as_secs_f64()
    );
    print_peak_memory();
    Ok(())
}

fn run_route(args: &[String]) -> Result<(), String> {
    let mut osm: Option<PathBuf> = None;
    let mut graph_path: Option<PathBuf> = None;
    let mut from: Option<(f64, f64)> = None;
    let mut to: Option<(f64, f64)> = None;
    let mut profile = Profile::Car;

    let mut index = 0;
    while index < args.len() {
        let flag = args[index].as_str();
        match flag {
            "--osm" => {
                osm = Some(PathBuf::from(flag_value(args, index, flag)?));
                index += 2;
            }
            "--graph" => {
                graph_path = Some(PathBuf::from(flag_value(args, index, flag)?));
                index += 2;
            }
            "--from" => {
                from = Some(parse_latlon(flag_value(args, index, flag)?)?);
                index += 2;
            }
            "--to" => {
                to = Some(parse_latlon(flag_value(args, index, flag)?)?);
                index += 2;
            }
            "--profile" => {
                profile = parse_profile(flag_value(args, index, flag)?)?;
                index += 2;
            }
            "--help" | "-h" => {
                println!("{USAGE}");
                return Ok(());
            }
            other => return Err(format!("unknown argument `{other}`")),
        }
    }

    let from = from.ok_or("`--from` is required")?;
    let to = to.ok_or("`--to` is required")?;
    let graph = match (graph_path, osm) {
        (Some(_), Some(_)) => {
            return Err("use either `--graph` or `--osm`, not both".to_string());
        }
        (Some(path), None) => {
            let load_started = Instant::now();
            let graph = Graph::load(&path).map_err(|err| err.to_string())?;
            let elapsed = load_started.elapsed();
            let bytes = fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
            eprintln!(
                "graph: {} nodes, {} edges, {} names, max speed {} km/h, loaded from {} ({}) in {:.3} s",
                graph.node_count(),
                graph.edge_count(),
                graph.names().len(),
                graph.max_speed_kmh(),
                path.display(),
                format_bytes(bytes),
                elapsed.as_secs_f64()
            );
            graph
        }
        (None, Some(path)) => {
            let load_started = Instant::now();
            let graph = load_pbf(&path).map_err(|err| err.to_string())?;
            print_graph_stats(&graph, load_started.elapsed(), "built from PBF");
            graph
        }
        (None, None) => {
            return Err("one of `--graph` or `--osm` is required".to_string());
        }
    };

    let route_started = Instant::now();
    let route = route(&graph, from, to, profile).map_err(|err| err.to_string())?;
    let route_elapsed = route_started.elapsed();
    eprintln!(
        "route: {:.1} km, {:.1} min, {} points, {} nodes, {} maneuvers, solved in {:.1} ms",
        route.distance_m / 1000.0,
        route.duration_s / 60.0,
        route.points.len(),
        route.nodes.len(),
        route.maneuvers.len(),
        route_elapsed.as_secs_f64() * 1000.0
    );
    print_peak_memory();

    let json = serde_json::to_string_pretty(&route).map_err(|err| err.to_string())?;
    println!("{json}");
    Ok(())
}

fn flag_value<'a>(args: &'a [String], index: usize, flag: &str) -> Result<&'a str, String> {
    args.get(index + 1)
        .map(String::as_str)
        .ok_or_else(|| format!("missing value for `{flag}`"))
}

fn parse_latlon(value: &str) -> Result<(f64, f64), String> {
    let (lat, lon) = value
        .split_once(',')
        .ok_or_else(|| format!("expected `lat,lon`, got `{value}`"))?;
    let lat: f64 = lat
        .trim()
        .parse()
        .map_err(|_| format!("invalid latitude in `{value}`"))?;
    let lon: f64 = lon
        .trim()
        .parse()
        .map_err(|_| format!("invalid longitude in `{value}`"))?;
    if !(-90.0..=90.0).contains(&lat) {
        return Err(format!("latitude out of range in `{value}`"));
    }
    if !(-180.0..=180.0).contains(&lon) {
        return Err(format!("longitude out of range in `{value}`"));
    }
    Ok((lat, lon))
}

fn parse_profile(name: &str) -> Result<Profile, String> {
    Profile::from_name(name).ok_or_else(|| format!("unknown profile `{name}` (supported: car)"))
}

fn print_graph_stats(graph: &Graph, elapsed: Duration, source: &str) {
    eprintln!(
        "graph: {} nodes, {} edges, {} names, max speed {} km/h, {source} in {:.2} s",
        graph.node_count(),
        graph.edge_count(),
        graph.names().len(),
        graph.max_speed_kmh(),
        elapsed.as_secs_f64()
    );
}

fn print_peak_memory() {
    if let Some(bytes) = peak_working_set_bytes() {
        eprintln!("peak working set: {}", format_bytes(bytes));
    }
}

fn format_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    let value = bytes as f64;
    if value >= GIB {
        format!("{:.2} GiB", value / GIB)
    } else if value >= MIB {
        format!("{:.1} MiB", value / MIB)
    } else {
        format!("{:.1} KiB", value / KIB)
    }
}

#[cfg(windows)]
fn peak_working_set_bytes() -> Option<u64> {
    use std::ffi::c_void;

    #[repr(C)]
    struct ProcessMemoryCounters {
        cb: u32,
        page_fault_count: u32,
        peak_working_set_size: usize,
        working_set_size: usize,
        quota_peak_paged_pool_usage: usize,
        quota_paged_pool_usage: usize,
        quota_peak_non_paged_pool_usage: usize,
        quota_non_paged_pool_usage: usize,
        pagefile_usage: usize,
        peak_pagefile_usage: usize,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut c_void;
        fn K32GetProcessMemoryInfo(
            process: *mut c_void,
            counters: *mut ProcessMemoryCounters,
            cb: u32,
        ) -> i32;
    }

    let mut counters = ProcessMemoryCounters {
        cb: std::mem::size_of::<ProcessMemoryCounters>() as u32,
        page_fault_count: 0,
        peak_working_set_size: 0,
        working_set_size: 0,
        quota_peak_paged_pool_usage: 0,
        quota_paged_pool_usage: 0,
        quota_peak_non_paged_pool_usage: 0,
        quota_non_paged_pool_usage: 0,
        pagefile_usage: 0,
        peak_pagefile_usage: 0,
    };
    let size = counters.cb;
    let ok = unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, size) };
    (ok != 0).then_some(counters.peak_working_set_size as u64)
}

#[cfg(target_os = "linux")]
fn peak_working_set_bytes() -> Option<u64> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    status.lines().find_map(|line| {
        let value = line.strip_prefix("VmHWM:")?.split_whitespace().next()?;
        let kib: u64 = value.parse().ok()?;
        Some(kib * 1024)
    })
}

#[cfg(not(any(windows, target_os = "linux")))]
fn peak_working_set_bytes() -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_latlon_with_spaces() {
        assert_eq!(parse_latlon("47.01, 28.86").unwrap(), (47.01, 28.86));
    }

    #[test]
    fn rejects_malformed_or_out_of_range_coordinates() {
        assert!(parse_latlon("47.01").is_err());
        assert!(parse_latlon("north,28.86").is_err());
        assert!(parse_latlon("91,28.86").is_err());
        assert!(parse_latlon("47.01,181").is_err());
    }

    #[test]
    fn formats_binary_byte_units() {
        assert_eq!(format_bytes(1024), "1.0 KiB");
        assert_eq!(format_bytes(2 * 1024 * 1024), "2.0 MiB");
    }

    #[test]
    fn help_is_not_an_error() {
        assert!(run(&["--help".to_string()]).is_ok());
    }

    #[test]
    fn unknown_command_is_an_error() {
        assert!(run(&["drive".to_string()]).is_err());
    }

    #[test]
    fn build_requires_both_osm_and_out() {
        assert!(run(&["build".to_string()]).is_err());
        assert!(run(&[
            "build".to_string(),
            "--osm".to_string(),
            "moldova.osm.pbf".to_string(),
        ])
        .is_err());
    }

    #[test]
    fn route_rejects_ambiguous_or_missing_graph_input() {
        let base = |extra: &[&str]| {
            let mut args = vec![
                "route".to_string(),
                "--from".to_string(),
                "47.0,28.0".to_string(),
                "--to".to_string(),
                "47.1,28.1".to_string(),
            ];
            args.extend(extra.iter().map(|value| (*value).to_string()));
            args
        };
        assert!(run(&base(&[])).is_err());
        assert!(run(&base(&["--graph", "g.bin", "--osm", "m.osm.pbf"])).is_err());
    }
}
