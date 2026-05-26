// Serez package manager
//
// Package layout on disk:
//   $SEREZ_PACKAGES/   (or ~/.serez/packages/)
//     <name>/
//       index.sz        ← default entry point
//       <submod>.sz     ← named submodule
//
// serez.json (project manifest):
//   { "name": "...", "version": "...", "dependencies": { "pkg": "version", ... } }
//
// Registry layout (SEREZ_REGISTRY env var or ~/.serez/registry/):
//   <name>/
//     <version>/
//       index.sz

use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ── Manifest ──────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct SerezManifest {
    pub name: String,
    pub version: String,
    pub dependencies: HashMap<String, String>,
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
        let mut dependencies: HashMap<String, String> = HashMap::new();

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
                        "name"    => name = val,
                        "version" => version = val,
                        _         => {} // unknown fields ignored
                    }
                }
                Some('{') => {
                    if key == "dependencies" {
                        dependencies = parse_string_map(&mut chars)?;
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
        Ok(SerezManifest { name, version, dependencies })
    }
}

// ── Package resolution ────────────────────────────────────────────────────────

/// Returns the user's package directory: $SEREZ_PACKAGES or ~/.serez/packages/
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

/// Install a package from the registry into the packages directory.
/// pkg_spec = "name" or "name@version".
pub fn install_package(pkg_spec: &str) -> Result<(), String> {
    let (pkg_name, pkg_version) = parse_pkg_spec(pkg_spec);
    let registry = registry_dir();

    // Find the version to install
    let version = if let Some(v) = pkg_version {
        v
    } else {
        // Use the highest version in the registry
        let pkg_reg_dir = registry.join(&pkg_name);
        if !pkg_reg_dir.exists() {
            return Err(format!(
                "Package '{}' not found in registry at {}",
                pkg_name,
                registry.display()
            ));
        }
        find_latest_version(&pkg_reg_dir)
            .ok_or_else(|| format!("No versions of '{}' found in registry", pkg_name))?
    };

    let src = registry.join(&pkg_name).join(&version);
    if !src.exists() {
        return Err(format!(
            "Package '{}@{}' not found in registry at {}",
            pkg_name, version,
            src.display()
        ));
    }

    let dest = packages_dir().join(&pkg_name);
    if dest.exists() {
        println!("Package '{}' already installed, updating...", pkg_name);
        std::fs::remove_dir_all(&dest)
            .map_err(|e| format!("Failed to remove old version: {}", e))?;
    }

    copy_dir_recursive(&src, &dest)
        .map_err(|e| format!("Failed to install '{}@{}': {}", pkg_name, version, e))?;

    println!("✅ Installed {}@{}", pkg_name, version);
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
