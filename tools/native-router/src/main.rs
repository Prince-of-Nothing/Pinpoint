//! `native-router` CLI: build a car graph from a local PBF and print one route
//! as JSON. Diagnostics go to stderr so stdout stays machine-readable.

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use native_router::{load_pbf, route, Profile};

const USAGE: &str = "\
Usage:
  native-router route --osm <file.osm.pbf> --from <lat,lon> --to <lat,lon> [--profile car]
  native-router --help

The route is computed locally from the PBF. No HTTP or online service is used.";

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
        Some("route") => run_route(&args[1..]),
        Some(other) => Err(format!("unknown command `{other}`")),
    }
}

fn run_route(args: &[String]) -> Result<(), String> {
    let mut osm: Option<PathBuf> = None;
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
            "--from" => {
                from = Some(parse_latlon(flag_value(args, index, flag)?)?);
                index += 2;
            }
            "--to" => {
                to = Some(parse_latlon(flag_value(args, index, flag)?)?);
                index += 2;
            }
            "--profile" => {
                let name = flag_value(args, index, flag)?;
                profile = Profile::from_name(name)
                    .ok_or_else(|| format!("unknown profile `{name}` (supported: car)"))?;
                index += 2;
            }
            "--help" | "-h" => {
                println!("{USAGE}");
                return Ok(());
            }
            other => return Err(format!("unknown argument `{other}`")),
        }
    }

    let osm = osm.ok_or("`--osm` is required")?;
    let from = from.ok_or("`--from` is required")?;
    let to = to.ok_or("`--to` is required")?;

    let load_started = Instant::now();
    let graph = load_pbf(&osm).map_err(|err| err.to_string())?;
    let load_elapsed = load_started.elapsed();
    eprintln!(
        "graph: {} nodes, {} edges, max speed {} km/h, loaded in {:.2} s",
        graph.node_count(),
        graph.edge_count(),
        graph.max_speed_kmh(),
        load_elapsed.as_secs_f64()
    );

    let route_started = Instant::now();
    let route = route(&graph, from, to, profile).map_err(|err| err.to_string())?;
    let route_elapsed = route_started.elapsed();
    eprintln!(
        "route: {:.1} km, {:.1} min, {} points, {} nodes, solved in {:.1} ms",
        route.distance_m / 1000.0,
        route.duration_s / 60.0,
        route.points.len(),
        route.nodes.len(),
        route_elapsed.as_secs_f64() * 1000.0
    );

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
    fn help_is_not_an_error() {
        assert!(run(&["--help".to_string()]).is_ok());
    }

    #[test]
    fn unknown_command_is_an_error() {
        assert!(run(&["drive".to_string()]).is_err());
    }
}
