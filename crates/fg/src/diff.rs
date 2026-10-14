//! `fg diff`: the differences between two guides, as text (or JSON).

use anyhow::{Context, Result, bail};
use fg_route::Route;
use fg_route::export::diff::{self, Change};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// A guide to compare: its route, what to call it, and the request and data it was made with
/// when known (a version saved by the app).
struct Side {
    name: String,
    route: Route,
    settings: Option<Value>,
}

/// Compare `a` and `b`: versions saved by the app (their folder `v3`, or its `outcome.json`),
/// routes of `fg route` (a file or a name in `routes_dir`). `a` alone, a guide folder of the app
/// (with `guide.json`): its two latest versions.
pub fn run(routes_dir: &Path, a: &Path, b: Option<&Path>, json: bool) -> Result<()> {
    let (a, b) = if let Some(b) = b {
        (load(routes_dir, a)?, load(routes_dir, b)?)
    } else {
        let [x, y] = latest_two(a)?;
        (load(routes_dir, &x)?, load(routes_dir, &y)?)
    };
    let d = diff::diff(&a.route, &b.route);
    if json {
        println!("{}", serde_json::to_string_pretty(&d)?);
        return Ok(());
    }
    let settings: Vec<Change> = match (&a.settings, &b.settings) {
        (Some(x), Some(y)) => diff::value_changes(x, y),
        _ => Vec::new(),
    };
    print!(
        "{}",
        diff::to_text(&d, &a.route, &b.route, [&a.name, &b.name], &settings)
    );
    Ok(())
}

/// The two latest versions in a guide folder of the app, oldest first.
fn latest_two(guide: &Path) -> Result<[PathBuf; 2]> {
    if !guide.join("guide.json").is_file() {
        bail!(
            "{}: not a guide folder of the app (give two versions to compare)",
            guide.display()
        );
    }
    let mut versions: Vec<u32> = std::fs::read_dir(guide)?
        .flatten()
        .filter_map(|e| e.file_name().to_str()?.strip_prefix('v')?.parse().ok())
        .collect();
    versions.sort_unstable();
    match versions[..] {
        [.., x, y] => Ok([guide.join(format!("v{x}")), guide.join(format!("v{y}"))]),
        _ => bail!("{}: fewer than two versions", guide.display()),
    }
}

fn load(routes_dir: &Path, path: &Path) -> Result<Side> {
    let mut file = path.to_path_buf();
    if file.is_dir() {
        file = file.join("outcome.json");
    } else if !file.exists() {
        file = routes_dir.join(format!("{}.json", fg_route::job::slug(&path.to_string_lossy())));
    }
    let read = |p: &Path| -> Result<Value> {
        serde_json::from_str(&std::fs::read_to_string(p).with_context(|| format!("reading {}", p.display()))?)
            .with_context(|| format!("parsing {}", p.display()))
    };
    let mut value = read(&file)?;
    // An outcome holds the route; a route of `fg route` is one.
    if let Some(route) = value.get_mut("route") {
        value = route.take();
    }
    let route = fg_route::export::read_route(value)?;
    // A version of the app: its number, request and data next to it.
    let folder = file.parent().unwrap_or(Path::new("."));
    let meta = read(&folder.join("meta.json")).ok();
    let name = meta.as_ref().and_then(|m| m.get("version")?.as_u64()).map_or_else(
        || file.file_stem().unwrap_or_default().to_string_lossy().into_owned(),
        |v| format!("v{v}"),
    );
    // Read as the app reads it: settings added since then have their default value.
    let request = read(&folder.join("request.json"))
        .ok()
        .and_then(|r| serde_json::from_value::<fg_route::job::PlanRequest>(r).ok())
        .and_then(|r| serde_json::to_value(r).ok());
    let settings = request.map(|mut request| {
        if let (Value::Object(r), Some(data)) = (&mut request, meta.as_ref().and_then(|m| m.get("data"))) {
            r.insert("data".into(), data.clone());
        }
        request
    });
    Ok(Side { name, route, settings })
}
