//! Managed CEF install under the application data directory.
//!
//! Production builds do not ship `libcef` inside the app bundle. Users (or the
//! auto-update component hook) download a pinned CEF artifact into
//! `paths::data_dir()/cef/<version>/`, which [`probe_libcef_path`] prefers.

use std::env;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context as _, Result, anyhow, bail};
use http_client::{AsyncBody, HttpClient};
use smol::fs;
use smol::stream::StreamExt as _;

/// Pinned CEF package version for managed installs (Spotify CDN builds).
pub const MANAGED_CEF_VERSION: &str = "131.3.5+g573cec5+chromium-131.0.6778.205";

const DOWNLOAD_URL_ENV: &str = "ZETA_CEF_DOWNLOAD_URL";

/// Resolve the managed install root: `data_dir/cef/<version>`.
pub fn managed_cef_root() -> PathBuf {
    paths::data_dir()
        .join("cef")
        .join(sanitize_version(MANAGED_CEF_VERSION))
}

fn sanitize_version(version: &str) -> String {
    version
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

/// Platform-specific relative path to the loadable CEF library inside a managed tree.
pub fn managed_libcef_relative_path() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "Chromium Embedded Framework.framework/Chromium Embedded Framework"
    }
    #[cfg(target_os = "linux")]
    {
        "libcef.so"
    }
    #[cfg(target_os = "windows")]
    {
        "libcef.dll"
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        "libcef"
    }
}

/// Return the managed libcef path when the file already exists.
pub fn probe_managed_libcef_path() -> Option<PathBuf> {
    let path = managed_cef_root().join(managed_libcef_relative_path());
    path.is_file().then_some(path)
}

/// Default download URL for the pinned CEF artifact (overridable via env).
pub fn default_cef_download_url() -> Result<String> {
    if let Ok(url) = env::var(DOWNLOAD_URL_ENV) {
        if url.trim().is_empty() {
            bail!("{DOWNLOAD_URL_ENV} is set but empty");
        }
        return Ok(url);
    }

    let platform = spotify_cef_platform()?;
    // Official Spotify CEF CDN; override with ZETA_CEF_DOWNLOAD_URL for a private mirror.
    // Use the minimal distribution: runtime binaries without samples/debug symbols.
    let version = MANAGED_CEF_VERSION.replace('+', "%2B");
    Ok(format!(
        "https://cef-builds.spotifycdn.com/cef_binary_{version}_{platform}_minimal.tar.bz2"
    ))
}

fn spotify_cef_platform() -> Result<&'static str> {
    match (env::consts::OS, env::consts::ARCH) {
        ("macos", "aarch64") => Ok("macosarm64"),
        ("macos", "x86_64") => Ok("macosx64"),
        ("linux", "x86_64") => Ok("linux64"),
        ("linux", "aarch64") => Ok("linuxarm64"),
        ("windows", "x86_64") => Ok("windows64"),
        ("windows", "aarch64") => Ok("windowsarm64"),
        (os, arch) => bail!("unsupported OS/arch for managed CEF: {os}/{arch}"),
    }
}

/// Ensure the pinned CEF runtime is present, downloading when missing.
///
/// Used by the explicit **Install Browser Runtime** action.
pub async fn install_if_needed(http: Arc<dyn HttpClient>) -> Result<PathBuf> {
    if let Some(path) = probe_managed_libcef_path() {
        log::info!("Managed CEF already installed at {}", path.display());
        return Ok(path);
    }

    download_and_extract(http).await
}

/// Component auto-update path: refresh only if the user previously installed CEF.
///
/// Fresh installs never auto-download; that keeps component updates off the
/// full-application auto-update path and avoids surprise large downloads.
pub async fn refresh_if_installed(http: Arc<dyn HttpClient>) -> Result<Option<PathBuf>> {
    let cef_root = paths::data_dir().join("cef");
    if !cef_root.exists() {
        return Ok(None);
    }
    if let Some(path) = probe_managed_libcef_path() {
        return Ok(Some(path));
    }
    Ok(Some(download_and_extract(http).await?))
}

async fn download_and_extract(http: Arc<dyn HttpClient>) -> Result<PathBuf> {
    let url = default_cef_download_url()?;
    log::info!("Downloading managed CEF from {url}");

    let root = managed_cef_root();
    let parent = root
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| paths::data_dir().join("cef"));
    fs::create_dir_all(&parent)
        .await
        .with_context(|| format!("creating {}", parent.display()))?;

    if root.exists() {
        fs::remove_dir_all(&root)
            .await
            .with_context(|| format!("removing stale {}", root.display()))?;
    }
    fs::create_dir_all(&root)
        .await
        .with_context(|| format!("creating {}", root.display()))?;

    let mut response = http
        .get(&url, AsyncBody::default(), true)
        .await
        .with_context(|| format!("downloading CEF from {url}"))?;
    if !response.status().is_success() {
        bail!(
            "CEF download failed with HTTP {} from {url}",
            response.status()
        );
    }

    let archive_path = parent.join(format!(
        "cef-{}-download.tar.bz2",
        sanitize_version(MANAGED_CEF_VERSION),
    ));
    {
        let mut file = fs::File::create(&archive_path)
            .await
            .with_context(|| format!("creating {}", archive_path.display()))?;
        futures::io::copy(response.body_mut(), &mut file)
            .await
            .context("writing CEF archive")?;
    }

    extract_archive(&archive_path, &root).await?;
    let _ = fs::remove_file(&archive_path).await;
    normalize_extracted_cef(&root).await?;

    let lib_path = root.join(managed_libcef_relative_path());
    if !lib_path.is_file() {
        bail!(
            "CEF archive extracted to {} but {} is missing",
            root.display(),
            lib_path.display()
        );
    }

    log::info!("Installed managed CEF at {}", lib_path.display());
    Ok(lib_path)
}

/// Flatten Spotify CEF layout (`cef_binary_*/Release/...`) into the managed root.
async fn normalize_extracted_cef(root: &Path) -> Result<()> {
    let expected = root.join(managed_libcef_relative_path());
    if expected.is_file() {
        return Ok(());
    }

    let release_dir = find_release_dir(root)
        .await?
        .ok_or_else(|| anyhow!("CEF archive missing Release/ under {}", root.display()))?;

    #[cfg(target_os = "macos")]
    {
        let framework_name = "Chromium Embedded Framework.framework";
        let source = release_dir.join(framework_name);
        if !source.is_dir() {
            bail!(
                "CEF Release/ missing {} at {}",
                framework_name,
                source.display()
            );
        }
        let destination = root.join(framework_name);
        fs::rename(&source, &destination)
            .await
            .with_context(|| format!("moving {} -> {}", source.display(), destination.display()))?;
        // CEF's GPU process resolves ANGLE/SwiftShader next to the helper /
        // framework_dir parent. Symlink the framework Libraries so software and
        // hardware paths can load them after we flatten Spotify's Release/.
        link_macos_gpu_libraries(root, &destination).await?;
    }

    #[cfg(not(target_os = "macos"))]
    {
        let mut entries = fs::read_dir(&release_dir)
            .await
            .with_context(|| format!("reading {}", release_dir.display()))?;
        while let Some(entry) = entries.next().await {
            let entry = entry?;
            let source = entry.path();
            let file_name = entry.file_name();
            let destination = root.join(&file_name);
            fs::rename(&source, &destination).await.with_context(|| {
                format!("moving {} -> {}", source.display(), destination.display())
            })?;
        }
    }

    remove_extraneous_extract_dirs(root).await?;
    Ok(())
}

async fn find_release_dir(root: &Path) -> Result<Option<PathBuf>> {
    let direct = root.join("Release");
    if direct.is_dir() {
        return Ok(Some(direct));
    }

    let mut entries = fs::read_dir(root)
        .await
        .with_context(|| format!("reading {}", root.display()))?;
    while let Some(entry) = entries.next().await {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let candidate = path.join("Release");
        if candidate.is_dir() {
            return Ok(Some(candidate));
        }
    }
    Ok(None)
}

async fn remove_extraneous_extract_dirs(root: &Path) -> Result<()> {
    let keep = match env::consts::OS {
        "macos" => "Chromium Embedded Framework.framework",
        _ => return Ok(()),
    };

    let mut entries = fs::read_dir(root)
        .await
        .with_context(|| format!("reading {}", root.display()))?;
    while let Some(entry) = entries.next().await {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if name == keep || is_macos_gpu_library_name(name) {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            fs::remove_dir_all(&path)
                .await
                .with_context(|| format!("removing leftover {}", path.display()))?;
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn is_macos_gpu_library_name(name: &str) -> bool {
    matches!(
        name,
        "libEGL.dylib" | "libGLESv2.dylib" | "libvk_swiftshader.dylib" | "vk_swiftshader_icd.json"
    )
}

#[cfg(not(target_os = "macos"))]
fn is_macos_gpu_library_name(_name: &str) -> bool {
    false
}

#[cfg(target_os = "macos")]
async fn link_macos_gpu_libraries(root: &Path, framework_dir: &Path) -> Result<()> {
    let libraries = framework_dir.join("Libraries");
    for name in [
        "libEGL.dylib",
        "libGLESv2.dylib",
        "libvk_swiftshader.dylib",
        "vk_swiftshader_icd.json",
    ] {
        let source = libraries.join(name);
        if !source.is_file() {
            continue;
        }
        let destination = root.join(name);
        if destination.exists() || destination.symlink_metadata().is_ok() {
            continue;
        }
        let relative = PathBuf::from("Chromium Embedded Framework.framework/Libraries").join(name);
        std::os::unix::fs::symlink(&relative, &destination).with_context(|| {
            format!(
                "symlinking {} -> {}",
                destination.display(),
                relative.display()
            )
        })?;
    }
    Ok(())
}

async fn extract_archive(archive_path: &Path, destination: &Path) -> Result<()> {
    // CEF frameworks ship symlink/hardlink layouts that async-tar fails on when
    // applying mtimes (ENOENT on libGLESv2.dylib etc.). System tar handles this.
    let status = smol::process::Command::new("tar")
        .arg("-xf")
        .arg(archive_path)
        .arg("-C")
        .arg(destination)
        .status()
        .await
        .with_context(|| format!("running tar to extract {}", archive_path.display()))?;
    if !status.success() {
        bail!("tar failed extracting {}", archive_path.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_replaces_plus_and_spaces() {
        assert_eq!(
            sanitize_version("131.3.5+g437feba+chromium-131"),
            "131.3.5_g437feba_chromium-131"
        );
    }

    #[test]
    fn managed_root_includes_version() {
        let root = managed_cef_root();
        assert!(
            root.ends_with(sanitize_version(MANAGED_CEF_VERSION)),
            "{}",
            root.display()
        );
    }

    #[test]
    fn default_url_points_at_spotify_minimal() {
        let url = default_cef_download_url().expect("url");
        assert!(
            url.starts_with("https://cef-builds.spotifycdn.com/cef_binary_"),
            "{url}"
        );
        assert!(url.contains("%2B"), "{url}");
        assert!(url.ends_with("_minimal.tar.bz2"), "{url}");
        assert!(!url.contains("digitaloceanspaces.com"), "{url}");
    }
}
