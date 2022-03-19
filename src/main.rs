//! Test the openapiv3 crate by trying to process all files from the
//! APIs-guru/openapi-directory repo.

use std::sync::{atomic::AtomicUsize, Arc};

use openapiv3::OpenAPI;
use rayon::iter::{ParallelBridge, ParallelIterator};
use semver::{Version, VersionReq};
use serde::Deserialize;

#[derive(Deserialize, Clone)]
struct Header {
    openapi: Option<String>,
    swagger: Option<String>,
}

enum Kind {
    Yaml,
    Json,
}

fn main() {
    // The openapiv3 crate can only handle [3.0.0..3.1.0)
    let req = VersionReq::parse(">=3.0.0, <3.1.0").unwrap();

    #[derive(Debug, Default)]
    struct Stats {
        invalid_file: AtomicUsize,
        invalid_format: AtomicUsize,
        invalid_version: AtomicUsize,
        failure: AtomicUsize,
        success: AtomicUsize,
    }

    let stats = Arc::<Stats>::default();

    walkdir::WalkDir::new("openapi-directory/APIs")
        .into_iter()
        // Use Rayon to parallelize the work.
        .par_bridge()
        .filter_map(|entry| match entry {
            Ok(entry) if entry.file_type().is_file() => Some(entry),
            _ => None,
        })
        .for_each(|entry| {
            let stats = stats.clone();
            let name = entry.path().to_string_lossy();
            let kind = if name.ends_with(".yaml") {
                Kind::Yaml
            } else if name.ends_with(".json") {
                Kind::Json
            } else {
                return;
            };

            let contents = std::fs::read_to_string(entry.path()).unwrap();
            let header = match kind {
                Kind::Yaml => {
                    serde_yaml::from_str::<Header>(contents.as_str()).map_err(|e| e.to_string())
                }
                Kind::Json => {
                    serde_json::from_str::<Header>(contents.as_str()).map_err(|e| e.to_string())
                }
            };

            match header.clone().map(|h| h.openapi) {
                Err(e) => {
                    println!("{} {} ❌", name, e);
                    stats
                        .invalid_file
                        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                }
                Ok(None) => {
                    println!("{} not openapi ({:?})", name, header.unwrap().swagger);
                    stats
                        .invalid_format
                        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                }
                Ok(Some(version)) if !req.matches(&Version::parse(version.as_str()).unwrap()) => {
                    println!("{} {} ❌", name, version);
                    stats
                        .invalid_version
                        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                }
                Ok(Some(version)) => {
                    let full = match kind {
                        Kind::Yaml => serde_yaml::from_str::<OpenAPI>(contents.as_str())
                            .map_err(|e| e.to_string()),
                        Kind::Json => serde_json::from_str::<OpenAPI>(contents.as_str())
                            .map_err(|e| e.to_string()),
                    };

                    match full {
                        Ok(_) => {
                            println!("{} {} ✅", name, version);
                            stats
                                .success
                                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        }
                        Err(e) => {
                            println!("{} {} 🐞", name, e);
                            stats
                                .failure
                                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        }
                    }
                }
            }
        });

    println!("{:?}", stats.as_ref());
}
