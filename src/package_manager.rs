// Serez package manager
//
// Package layout on disk:
//   $SEREZ_PACKAGES/   (or ~/.serez/packages/)
//     <name>/
//       index.sz        ← default entry point
//       <submod>.sz     ← named submodule
//
// serez.json (project manifest):
//   { "name": "...", "version": "...", "description": "...", "author": "...", "dependencies": { "pkg": "version", ... } }
//
// Registry layout (SEREZ_REGISTRY env var or ~/.serez/registry/):
//   <name>/
//     <version>/
//       index.sz
//
// HTTP registry (SEREZ_REGISTRY_URL env var or https://packages.serezcode.org):
//   GET  /api/packages/<name>/latest          → plain-text version string
//   GET  /api/packages/<name>/<version>.zip   → zip archive of package files
//   GET  /api/packages/<name>/stats           → JSON with download stats
//   POST /api/publish                         → publish a new package version
//   DEL  /api/unpublish/<name>/<version>      → yank a version

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

// ── Manifest ──────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct SerezManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub dependencies: HashMap<String, String>,
    pub permissions: Vec<String>,
    pub scripts: HashMap<String, String>,
}

impl SerezManifest {
    /// Read and parse `serez.json` from `dir`.
    pub fn load(dir: &Path) -> Result<SerezManifest, String> {
        let path = dir.join("serez.json");
        let raw = std::fs::read_to_string(&path)
            .map_err(|e| format!("Cannot read serez.json: {}", e))?;
        SerezManifest::parse(&raw)
    }

    fn parse(raw: &str) -> Result<SerezManifest, String> {
        // Minimal hand-rolled JSON parser for the specific manifest shape.
        let raw = raw.trim();
        if !raw.starts_with('{') || !raw.ends_with('}') {
            return Err("serez.json must be a JSON object".to_string());
        }
        let mut name = String::new();
        let mut version = String::new();
        let mut description = String::new();
        let mut author = String::new();
        let mut dependencies: HashMap<String, String> = HashMap::new();
        let mut permissions: Vec<String> = Vec::new();
        let mut scripts: HashMap<String, String> = HashMap::new();

        // Extract top-level string fields and the dependencies object.
        let inner = &raw[1..raw.len() - 1];

        // Simple tokenizer: extract quoted keys and values
        let mut chars = inner.chars().peekable();
        loop {
            // skip whitespace and commas
            while chars.peek().map_or(false, |c| c.is_whitespace() || *c == ',') {
                chars.next();
            }
            if chars.peek().is_none() { break; }

            // Expect a key
            if chars.peek() != Some(&'"') { break; }
            let key = read_json_string(&mut chars)?;

            // Expect ':'
            skip_ws_and(&mut chars, ':');

            // Either a quoted string or '{' for object
            match chars.peek() {
                Some('"') => {
                    let val = read_json_string(&mut chars)?;
                    match key.as_str() {
                        "name"        => name = val,
                        "version"     => version = val,
                        "description" => description = val,
                        "author"      => author = val,
                        _             => {}
                    }
                }
                Some('{') => {
                    if key == "dependencies" {
                        dependencies = parse_string_map(&mut chars)?;
                    } else if key == "scripts" {
                        scripts = parse_string_map(&mut chars)?;
                    } else {
                        skip_value(&mut chars);
                    }
                }
                Some('[') => {
                    if key == "permissions" {
                        permissions = parse_string_array(&mut chars)?;
                    } else {
                        skip_value(&mut chars);
                    }
                }
                _ => { skip_value(&mut chars); }
            }
        }

        if name.is_empty() {
            return Err("serez.json: 'name' field is required".to_string());
        }
        if version.is_empty() {
            return Err("serez.json: 'version' field is required".to_string());
        }
        Ok(SerezManifest { name, version, description, author, dependencies, permissions, scripts })
    }
}

// ── Package resolution ────────────────────────────────────────────────────────

/// Returns the local project package directory: <cwd>/packages/
pub fn local_packages_dir() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("packages")
}

/// Returns the global fallback package directory: $SEREZ_PACKAGES or ~/.serez/packages/
pub fn packages_dir() -> PathBuf {
    if let Ok(p) = std::env::var("SEREZ_PACKAGES") {
        return PathBuf::from(p);
    }
    if let Some(home) = home_dir() {
        return home.join(".serez").join("packages");
    }
    PathBuf::from(".serez/packages")
}

/// Returns the registry directory: $SEREZ_REGISTRY or ~/.serez/registry/
pub fn registry_dir() -> PathBuf {
    if let Ok(p) = std::env::var("SEREZ_REGISTRY") {
        return PathBuf::from(p);
    }
    if let Some(home) = home_dir() {
        return home.join(".serez").join("registry");
    }
    PathBuf::from(".serez/registry")
}

/// Returns the HTTP registry base URL: $SEREZ_REGISTRY_URL or the official registry.
pub fn registry_url() -> String {
    std::env::var("SEREZ_REGISTRY_URL")
        .unwrap_or_else(|_| "https://packages.serezcode.org".to_string())
}

/// Install a package from the local registry or HTTP registry into ./packages/.
/// pkg_spec = "name" or "name@version".
pub fn install_package(pkg_spec: &str) -> Result<(), String> {
    let (pkg_name, pkg_version) = parse_pkg_spec(pkg_spec);
    let registry = registry_dir();

    // Resolve version: explicit → local registry latest → HTTP latest
    let version = if let Some(v) = pkg_version {
        v
    } else {
        let pkg_reg_dir = registry.join(&pkg_name);
        if pkg_reg_dir.exists() {
            find_latest_version(&pkg_reg_dir)
                .ok_or_else(|| format!("No versions of '{}' found in local registry", pkg_name))?
        } else {
            fetch_latest_version(&pkg_name)?
        }
    };

    let dest = local_packages_dir().join(&pkg_name);
    if dest.exists() {
        println!("Package '{}' already installed, updating...", pkg_name);
        std::fs::remove_dir_all(&dest)
            .map_err(|e| format!("Failed to remove old version: {}", e))?;
    }

    // Try local registry first, fall back to HTTP
    let src = registry.join(&pkg_name).join(&version);
    if src.exists() {
        copy_dir_recursive(&src, &dest)
            .map_err(|e| format!("Failed to install '{}@{}': {}", pkg_name, version, e))?;
        println!("✅ Installed {}@{} → ./packages/{}", pkg_name, version, pkg_name);
    } else {
        download_package(&pkg_name, &version)?;
    }

    Ok(())
}

/// Execute a script defined in serez.json's "scripts" section.
pub fn run_script(script_name: &str) -> Result<(), String> {
    let cwd = std::env::current_dir()
        .map_err(|e| format!("Cannot get current directory: {}", e))?;
    let manifest = SerezManifest::load(&cwd)?;

    let cmd = manifest.scripts.get(script_name).cloned().ok_or_else(|| {
        let available: Vec<&String> = manifest.scripts.keys().collect();
        if available.is_empty() {
            format!("No scripts defined in serez.json")
        } else {
            format!(
                "Script '{}' not found. Available: {}",
                script_name,
                available.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
            )
        }
    })?;

    println!("▶ {}", cmd);

    let status = if cfg!(target_os = "windows") {
        std::process::Command::new("cmd")
            .args(["/C", &cmd])
            .current_dir(&cwd)
            .status()
    } else {
        std::process::Command::new("sh")
            .args(["-c", &cmd])
            .current_dir(&cwd)
            .status()
    }
    .map_err(|e| format!("Failed to execute script '{}': {}", script_name, e))?;

    if !status.success() {
        let code = status.code().unwrap_or(1);
        return Err(format!("Script '{}' exited with code {}", script_name, code));
    }
    Ok(())
}

/// Remove a package from the local project packages directory.
pub fn uninstall_package(pkg_name: &str) -> Result<(), String> {
    let dest = local_packages_dir().join(pkg_name);
    if !dest.exists() {
        return Err(format!("Package '{}' is not installed in ./packages/", pkg_name));
    }
    std::fs::remove_dir_all(&dest)
        .map_err(|e| format!("Failed to uninstall '{}': {}", pkg_name, e))?;
    println!("✅ Uninstalled {}", pkg_name);
    Ok(())
}

/// Install all dependencies listed in serez.json from the current directory.
pub fn install_all() -> Result<(), String> {
    let cwd = std::env::current_dir()
        .map_err(|e| format!("Cannot get current directory: {}", e))?;
    let manifest = SerezManifest::load(&cwd)?;

    if manifest.dependencies.is_empty() {
        println!("No dependencies to install.");
        return Ok(());
    }

    for (name, version) in &manifest.dependencies {
        let spec = format!("{}@{}", name, version);
        install_package(&spec)?;
    }
    Ok(())
}

// ── HTTP registry ─────────────────────────────────────────────────────────────

/// Fetch the latest version string for a package from the HTTP registry.
fn fetch_latest_version(pkg_name: &str) -> Result<String, String> {
    let url = format!("{}/api/packages/{}/latest", registry_url(), pkg_name);
    let response = ureq::get(&url).call().map_err(|e| match e {
        ureq::Error::Status(404, _) => format!(
            "Package '{}' not found in local registry or remote registry ({})",
            pkg_name, registry_url()
        ),
        other => format!("Failed to reach registry for '{}': {}", pkg_name, other),
    })?;
    let version = response
        .into_string()
        .map_err(|e| format!("Invalid response from registry: {}", e))?
        .trim()
        .to_string();
    if version.is_empty() {
        return Err(format!("Registry returned empty version for '{}'", pkg_name));
    }
    Ok(version)
}

/// Download a package zip from the HTTP registry and extract it to ./packages/<name>/.
fn download_package(pkg_name: &str, version: &str) -> Result<(), String> {
    let url = format!("{}/api/packages/{}/{}.zip", registry_url(), pkg_name, version);
    println!("Downloading {}@{} from {}...", pkg_name, version, registry_url());

    let response = ureq::get(&url).call().map_err(|e| match e {
        ureq::Error::Status(404, _) => format!(
            "Package '{}@{}' not found in remote registry",
            pkg_name, version
        ),
        other => format!("Download failed for '{}@{}': {}", pkg_name, version, other),
    })?;

    let mut bytes = Vec::new();
    response
        .into_reader()
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Failed to read download for '{}@{}': {}", pkg_name, version, e))?;

    let dest = local_packages_dir().join(pkg_name);
    extract_zip(&bytes, &dest)
        .map_err(|e| format!("Failed to extract '{}@{}': {}", pkg_name, version, e))?;

    println!("✅ Installed {}@{} → ./packages/{} (remote)", pkg_name, version, pkg_name);
    Ok(())
}

// ── Publish / Unpublish / Info ────────────────────────────────────────────────

/// Publish the package in the current directory to the registry.
/// Reads serez.json, zips the package directory recursively (honoring .szignore),
/// and POSTs to /api/publish.
pub fn publish_package() -> Result<(), String> {
    let cwd = std::env::current_dir()
        .map_err(|e| format!("Cannot get current directory: {}", e))?;
    let manifest = SerezManifest::load(&cwd)?;

    let api_key = std::env::var("SEREZ_API_KEY")
        .map_err(|_| "SEREZ_API_KEY environment variable not set.\nSet it with: export SEREZ_API_KEY=<your-key>".to_string())?;

    println!("Publishing {}@{} ...", manifest.name, manifest.version);

    let zip_bytes = create_package_zip(&cwd)?;
    let boundary = "SerezPkgBoundary7MA4YWxkTrZu0gW";
    let mut body: Vec<u8> = Vec::new();

    for (key, val) in &[
        ("name",        manifest.name.as_str()),
        ("version",     manifest.version.as_str()),
        ("description", manifest.description.as_str()),
        ("author",      manifest.author.as_str()),
    ] {
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(
            format!("Content-Disposition: form-data; name=\"{}\"\r\n\r\n", key).as_bytes(),
        );
        body.extend_from_slice(val.as_bytes());
        body.extend_from_slice(b"\r\n");
    }

    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(
        format!(
            "Content-Disposition: form-data; name=\"zip\"; filename=\"{}-{}.zip\"\r\nContent-Type: application/zip\r\n\r\n",
            manifest.name, manifest.version
        )
        .as_bytes(),
    );
    body.extend_from_slice(&zip_bytes);
    body.extend_from_slice(b"\r\n");
    body.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());

    let url = format!("{}/api/publish", registry_url());
    let ct  = format!("multipart/form-data; boundary={}", boundary);

    ureq::post(&url)
        .set("x-api-key", &api_key)
        .set("Content-Type", &ct)
        .send_bytes(&body)
        .map_err(|e| match e {
            ureq::Error::Status(401, _) => "Unauthorized — check your SEREZ_API_KEY".to_string(),
            ureq::Error::Status(409, _) => format!("Version {} already exists in the registry", manifest.version),
            ureq::Error::Status(400, r) => format!("Bad request: {}", r.into_string().unwrap_or_default()),
            other => format!("Publish failed: {}", other),
        })?;

    println!("✅ Published {}@{}", manifest.name, manifest.version);
    Ok(())
}

/// Remove a published version from the registry (yank).
/// pkg_spec = "name@version"
pub fn unpublish_package_remote(pkg_spec: &str) -> Result<(), String> {
    let (pkg_name, version) = parse_pkg_spec(pkg_spec);
    let version = version.ok_or_else(|| "Usage: sz unpublish <package>@<version>".to_string())?;

    let api_key = std::env::var("SEREZ_API_KEY")
        .map_err(|_| "SEREZ_API_KEY environment variable not set".to_string())?;

    let url = format!("{}/api/unpublish/{}/{}", registry_url(), pkg_name, version);

    ureq::delete(&url)
        .set("x-api-key", &api_key)
        .call()
        .map_err(|e| match e {
            ureq::Error::Status(401, _) => "Unauthorized — check your SEREZ_API_KEY".to_string(),
            ureq::Error::Status(404, _) => format!("{}@{} not found in registry", pkg_name, version),
            other => format!("Unpublish failed: {}", other),
        })?;

    println!("✅ Unpublished {}@{}", pkg_name, version);
    Ok(())
}

/// Show stats and version list for a package in the registry.
pub fn info_package(pkg_name: &str) -> Result<(), String> {
    let url = format!("{}/api/packages/{}/stats", registry_url(), pkg_name);
    let body = ureq::get(&url)
        .call()
        .map_err(|e| match e {
            ureq::Error::Status(404, _) => format!("Package '{}' not found", pkg_name),
            other => format!("Failed to reach registry: {}", other),
        })?
        .into_string()
        .map_err(|e| format!("Invalid response: {}", e))?;

    // Minimal display — extract numbers with basic string search
    let total   = extract_json_number(&body, "total").unwrap_or(0);
    let weekly  = extract_json_number(&body, "weekly").unwrap_or(0);
    let monthly = extract_json_number(&body, "monthly").unwrap_or(0);

    println!("\nPackage: {}", pkg_name);
    println!("  Total downloads:   {}", total);
    println!("  Weekly downloads:  {}", weekly);
    println!("  Monthly downloads: {}", monthly);

    // Extract versions array entries
    println!("\nVersions:");
    let mut search = body.as_str();
    while let Some(idx) = search.find("\"version\":") {
        search = &search[idx + 10..];
        if let Some(start) = search.find('"') {
            let inner = &search[start + 1..];
            if let Some(end) = inner.find('"') {
                let ver = &inner[..end];
                let yanked = search.contains("\"yanked\":1") || search.find("\"yanked\":1").map_or(false, |i| i < 60);
                if yanked {
                    println!("  {} (unpublished)", ver);
                } else {
                    println!("  {}", ver);
                }
            }
        }
    }
    println!();
    Ok(())
}

fn extract_json_number(json: &str, key: &str) -> Option<u64> {
    let needle = format!("\"{}\":", key);
    let idx = json.find(&needle)?;
    let after = json[idx + needle.len()..].trim_start();
    let end = after.find(|c: char| !c.is_ascii_digit()).unwrap_or(after.len());
    after[..end].parse().ok()
}

/// Zip all .sz files in dir into an in-memory buffer.
fn create_package_zip(dir: &Path) -> Result<Vec<u8>, String> {
    use std::io::Write;

    let patterns = read_szignore(dir);
    let mut files: Vec<(String, PathBuf)> = Vec::new();
    collect_package_files(dir, dir, &patterns, &mut files)?;
    files.sort();

    let buf = std::io::Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(buf);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    for (rel, path) in &files {
        zip.start_file(rel, options)
            .map_err(|e| format!("Zip error on '{}': {}", rel, e))?;
        let content = std::fs::read(path)
            .map_err(|e| format!("Cannot read '{}': {}", rel, e))?;
        zip.write_all(&content)
            .map_err(|e| format!("Zip write error: {}", e))?;
    }

    let buf = zip.finish().map_err(|e| format!("Zip finish error: {}", e))?;
    Ok(buf.into_inner())
}

/// Read patterns from `<dir>/.szignore` (gitignore-like). Blank lines and lines
/// starting with `#` are skipped. Returns an empty list if the file is absent.
fn read_szignore(dir: &Path) -> Vec<String> {
    match std::fs::read_to_string(dir.join(".szignore")) {
        Ok(s) => s
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Recursively collect files under `dir` (relative to `root`), skipping ignored
/// paths, `.git/` and the `.szignore` file itself. Fills `out` with
/// (relative_path_with_forward_slashes, absolute_path).
fn collect_package_files(
    root: &Path,
    dir: &Path,
    patterns: &[String],
    out: &mut Vec<(String, PathBuf)>,
) -> Result<(), String> {
    for entry in std::fs::read_dir(dir).map_err(|e| format!("Cannot read directory: {}", e))? {
        let entry = entry.map_err(|e| format!("Directory read error: {}", e))?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        // Never publish VCS metadata or the ignore file itself.
        if name == ".git" || name == ".szignore" {
            continue;
        }

        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");

        if is_ignored(&rel, patterns) {
            continue;
        }

        if path.is_dir() {
            collect_package_files(root, &path, patterns, out)?;
        } else if path.is_file() {
            out.push((rel, path));
        }
    }
    Ok(())
}

/// gitignore-like match. A pattern may be an exact name/path, a directory
/// (`apps/` or `/apps/`) which excludes it and everything under it, or a glob
/// with `*` (e.g. `*.txt`).
fn is_ignored(rel: &str, patterns: &[String]) -> bool {
    let rel = rel.trim_start_matches("./");
    for pat in patterns {
        let p = pat.trim_start_matches("./").trim_start_matches('/');
        if p.is_empty() {
            continue;
        }
        // Directory pattern: "apps/" → the dir and everything beneath it.
        if let Some(d) = p.strip_suffix('/') {
            if rel == d
                || rel.starts_with(&format!("{}/", d))
                || rel.split('/').any(|seg| seg == d)
            {
                return true;
            }
            continue;
        }
        // Glob pattern: matched against the basename and the full relative path.
        if p.contains('*') {
            let base = rel.rsplit('/').next().unwrap_or(rel);
            if glob_match(p, base) || glob_match(p, rel) {
                return true;
            }
            continue;
        }
        // Plain name/path: exact path, basename anywhere, or as a directory prefix.
        let base = rel.rsplit('/').next().unwrap_or(rel);
        if rel == p
            || base == p
            || rel.starts_with(&format!("{}/", p))
            || rel.split('/').any(|seg| seg == p)
        {
            return true;
        }
    }
    false
}

/// Minimal wildcard matcher supporting `*` (matches any run of characters).
fn glob_match(pattern: &str, text: &str) -> bool {
    fn helper(p: &[u8], t: &[u8]) -> bool {
        if p.is_empty() {
            return t.is_empty();
        }
        if p[0] == b'*' {
            helper(&p[1..], t) || (!t.is_empty() && helper(p, &t[1..]))
        } else if !t.is_empty() && p[0] == t[0] {
            helper(&p[1..], &t[1..])
        } else {
            false
        }
    }
    helper(pattern.as_bytes(), text.as_bytes())
}

/// Extract a zip archive into dest/, skipping any top-level directory wrapper.
fn extract_zip(data: &[u8], dest: &Path) -> Result<(), String> {
    let cursor = std::io::Cursor::new(data);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| format!("Invalid zip archive: {}", e))?;

    // Detect a single common top-level wrapper dir (e.g. "serez-ai-1.0.0/") shared
    // by ALL entries, and strip it. If entries don't share one prefix (files at the
    // root plus subdirs like src/), strip nothing — preserve the layout as-is.
    let names: Vec<String> = (0..archive.len())
        .map(|i| {
            archive
                .by_index(i)
                .map(|f| f.name().to_string())
                .unwrap_or_default()
        })
        .collect();
    let prefix: Option<String> = names
        .first()
        .and_then(|name| name.find('/').map(|i| name[..=i].to_string()))
        .filter(|p| names.iter().all(|n| n.starts_with(p.as_str())));

    std::fs::create_dir_all(dest)
        .map_err(|e| format!("Cannot create destination: {}", e))?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Zip read error at entry {}: {}", i, e))?;

        let raw_name = file.name().to_string();

        // Security: reject path traversal
        if raw_name.contains("..") {
            return Err(format!("Unsafe path in archive: '{}'", raw_name));
        }

        // Strip top-level prefix if present
        let rel = if let Some(ref pfx) = prefix {
            raw_name.strip_prefix(pfx.as_str()).unwrap_or(&raw_name)
        } else {
            &raw_name
        };

        if rel.is_empty() {
            continue;
        }

        let outpath = dest.join(rel);

        if raw_name.ends_with('/') {
            std::fs::create_dir_all(&outpath)
                .map_err(|e| format!("Cannot create dir '{}': {}", outpath.display(), e))?;
        } else {
            if let Some(parent) = outpath.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("Cannot create dir '{}': {}", parent.display(), e))?;
            }
            let mut content = Vec::new();
            file.read_to_end(&mut content)
                .map_err(|e| format!("Cannot read zip entry '{}': {}", raw_name, e))?;
            std::fs::write(&outpath, content)
                .map_err(|e| format!("Cannot write '{}': {}", outpath.display(), e))?;
        }
    }
    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn parse_pkg_spec(spec: &str) -> (String, Option<String>) {
    if let Some(idx) = spec.find('@') {
        (spec[..idx].to_string(), Some(spec[idx + 1..].to_string()))
    } else {
        (spec.to_string(), None)
    }
}

fn find_latest_version(pkg_reg_dir: &Path) -> Option<String> {
    std::fs::read_dir(pkg_reg_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .max()
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let dest_path = dest.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir_recursive(&entry.path(), &dest_path)?;
        } else {
            std::fs::copy(entry.path(), dest_path)?;
        }
    }
    Ok(())
}

fn home_dir() -> Option<PathBuf> {
    // Try HOME (Unix) or USERPROFILE (Windows)
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
        .map(PathBuf::from)
}

// ── JSON mini-parser helpers ──────────────────────────────────────────────────

fn read_json_string(chars: &mut std::iter::Peekable<std::str::Chars>) -> Result<String, String> {
    // expects '"' as next char
    if chars.next() != Some('"') {
        return Err("Expected '\"'".to_string());
    }
    let mut s = String::new();
    loop {
        match chars.next() {
            None | Some('\0') => return Err("Unterminated string".to_string()),
            Some('"') => break,
            Some('\\') => {
                match chars.next() {
                    Some('"')  => s.push('"'),
                    Some('\\') => s.push('\\'),
                    Some('n')  => s.push('\n'),
                    Some('t')  => s.push('\t'),
                    Some('r')  => s.push('\r'),
                    Some(c)    => { s.push('\\'); s.push(c); }
                    None       => return Err("Unterminated escape".to_string()),
                }
            }
            Some(c) => s.push(c),
        }
    }
    Ok(s)
}

fn skip_ws_and(chars: &mut std::iter::Peekable<std::str::Chars>, expect: char) {
    while chars.peek().map_or(false, |c| c.is_whitespace()) {
        chars.next();
    }
    if chars.peek() == Some(&expect) {
        chars.next();
    }
    while chars.peek().map_or(false, |c| c.is_whitespace()) {
        chars.next();
    }
}

fn parse_string_map(
    chars: &mut std::iter::Peekable<std::str::Chars>,
) -> Result<HashMap<String, String>, String> {
    // Consume '{'
    if chars.next() != Some('{') {
        return Err("Expected '{'".to_string());
    }
    let mut map = HashMap::new();
    loop {
        while chars.peek().map_or(false, |c| c.is_whitespace() || *c == ',') {
            chars.next();
        }
        match chars.peek() {
            None | Some('}') => { chars.next(); break; }
            Some('"') => {}
            _ => break,
        }
        let key = read_json_string(chars)?;
        skip_ws_and(chars, ':');
        let val = read_json_string(chars)?;
        map.insert(key, val);
    }
    Ok(map)
}

fn parse_string_array(
    chars: &mut std::iter::Peekable<std::str::Chars>,
) -> Result<Vec<String>, String> {
    if chars.next() != Some('[') {
        return Err("Expected '['".to_string());
    }
    let mut arr = Vec::new();
    loop {
        while chars.peek().map_or(false, |c| c.is_whitespace() || *c == ',') {
            chars.next();
        }
        match chars.peek() {
            None | Some(']') => { chars.next(); break; }
            Some('"') => {}
            _ => break,
        }
        arr.push(read_json_string(chars)?);
    }
    Ok(arr)
}

fn skip_value(chars: &mut std::iter::Peekable<std::str::Chars>) {
    // Minimal skip: handles strings, numbers, booleans, null, nested {}
    while chars.peek().map_or(false, |c| c.is_whitespace()) {
        chars.next();
    }
    match chars.peek() {
        Some('"') => { let _ = read_json_string(chars); }
        Some('{') => {
            let mut depth = 0i32;
            for c in chars.by_ref() {
                if c == '{' { depth += 1; }
                if c == '}' { depth -= 1; if depth == 0 { break; } }
            }
        }
        Some('[') => {
            let mut depth = 0i32;
            for c in chars.by_ref() {
                if c == '[' { depth += 1; }
                if c == ']' { depth -= 1; if depth == 0 { break; } }
            }
        }
        _ => {
            while chars.peek().map_or(false, |c| !matches!(c, ',' | '}' | ']')) {
                chars.next();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_parse() {
        let json = r#"{
          "name": "my-pkg",
          "version": "1.2.3",
          "dependencies": {
            "pkg-a": "0.1.0",
            "pkg-b": "2.0.0"
          }
        }"#;
        let m = SerezManifest::parse(json).unwrap();
        assert_eq!(m.name, "my-pkg");
        assert_eq!(m.version, "1.2.3");
        assert_eq!(m.dependencies["pkg-a"], "0.1.0");
        assert_eq!(m.dependencies["pkg-b"], "2.0.0");
    }

    #[test]
    fn test_manifest_no_deps() {
        let json = r#"{"name":"simple","version":"1.0.0","dependencies":{}}"#;
        let m = SerezManifest::parse(json).unwrap();
        assert_eq!(m.name, "simple");
        assert!(m.dependencies.is_empty());
    }

    #[test]
    fn test_pkg_spec_with_version() {
        let (name, ver) = parse_pkg_spec("foo@1.2.3");
        assert_eq!(name, "foo");
        assert_eq!(ver, Some("1.2.3".to_string()));
    }

    #[test]
    fn test_pkg_spec_without_version() {
        let (name, ver) = parse_pkg_spec("foo");
        assert_eq!(name, "foo");
        assert_eq!(ver, None);
    }
}
