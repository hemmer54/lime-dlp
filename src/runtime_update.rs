use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
};

use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zip::ZipArchive;

const GITHUB_API: &str = "https://api.github.com/repos";
const BUNDLE_MANIFEST: &str = "runtime-manifest.json";
const RUNTIME_FILES: [&str; 6] = [
    "yt-dlp.exe",
    "deno.exe",
    "ffmpeg.exe",
    "ffprobe.exe",
    "ffplay.exe",
    "yt.solver.deno.lib.js",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeVersions {
    pub yt_dlp: String,
    pub deno: String,
    pub ffmpeg: String,
    pub ejs: String,
}

impl std::fmt::Display for RuntimeVersions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "yt-dlp {}, Deno {}, FFmpeg {}, EJS {}",
            self.yt_dlp, self.deno, self.ffmpeg, self.ejs
        )
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("runtime updates are only supported on 64-bit Windows")]
    UnsupportedPlatform,
    #[error("failed to fetch upstream release metadata: {0}")]
    Request(#[from] reqwest::Error),
    #[error("runtime update file operation failed: {0}")]
    Io(#[from] io::Error),
    #[error("invalid release metadata: {0}")]
    InvalidMetadata(String),
    #[error("archive extraction failed: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("failed to read runtime manifest: {0}")]
    Manifest(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct RuntimeDigests {
    yt_dlp: String,
    deno: String,
    ffmpeg: String,
    ejs: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RuntimeManifest {
    schema: u32,
    versions: RuntimeVersions,
    digests: RuntimeDigests,
    #[serde(default)]
    file_digests: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    name: Option<String>,
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
    digest: Option<String>,
}

#[derive(Debug, Clone)]
struct DownloadAsset {
    url: String,
    digest: String,
}

#[derive(Debug, Clone)]
struct UpdatePlan {
    versions: RuntimeVersions,
    digests: RuntimeDigests,
    yt_dlp: DownloadAsset,
    deno: DownloadAsset,
    ffmpeg: DownloadAsset,
    ejs: DownloadAsset,
}

pub async fn check_for_update() -> Result<Option<RuntimeVersions>, Error> {
    if !is_supported_platform() {
        return Ok(None);
    }

    let plan = latest_plan().await?;
    let current_dir = active_bundle_dir().or_else(bundled_runtime_dir);
    let up_to_date = current_dir
        .as_deref()
        .is_some_and(|dir| bundle_matches(dir, &plan.digests));

    Ok((!up_to_date).then_some(plan.versions))
}

pub async fn install_latest() -> Result<RuntimeVersions, Error> {
    if !is_supported_platform() {
        return Err(Error::UnsupportedPlatform);
    }

    let plan = latest_plan().await?;
    let root = runtime_root().ok_or_else(|| {
        Error::InvalidMetadata("could not locate the per-user data directory".into())
    })?;
    let bundles_dir = root.join("bundles");
    fs::create_dir_all(&bundles_dir)?;

    let bundle_id = bundle_id(&plan.digests);
    let installed_dir = bundles_dir.join(&bundle_id);
    if bundle_matches(&installed_dir, &plan.digests) {
        activate_bundle(&root, &bundle_id)?;
        return Ok(plan.versions);
    }

    let staging_dir = root.join(format!(".staging-{bundle_id}"));
    if staging_dir.exists() {
        fs::remove_dir_all(&staging_dir)?;
    }
    fs::create_dir_all(&staging_dir)?;

    let client = api_client()?;
    let ytdlp_path = staging_dir.join("yt-dlp.exe");
    download_verified(&client, &plan.yt_dlp, &ytdlp_path, 100 * 1024 * 1024).await?;

    let deno_archive = staging_dir.join("deno.zip");
    download_verified(&client, &plan.deno, &deno_archive, 150 * 1024 * 1024).await?;
    extract_zip_file(&deno_archive, "deno.exe", &staging_dir.join("deno.exe"))?;
    fs::remove_file(deno_archive)?;

    let ffmpeg_archive = staging_dir.join("ffmpeg.zip");
    download_verified(&client, &plan.ffmpeg, &ffmpeg_archive, 400 * 1024 * 1024).await?;
    for executable in ["ffmpeg.exe", "ffprobe.exe", "ffplay.exe"] {
        extract_zip_file(
            &ffmpeg_archive,
            &format!("bin/{executable}"),
            &staging_dir.join(executable),
        )?;
    }
    fs::remove_file(ffmpeg_archive)?;

    let ejs_path = staging_dir.join("yt.solver.deno.lib.js");
    download_verified(&client, &plan.ejs, &ejs_path, 32 * 1024 * 1024).await?;

    if !runtime_files_present(&staging_dir) {
        return Err(Error::InvalidMetadata(
            "the downloaded bundle is missing required files".into(),
        ));
    }

    let file_digests = RUNTIME_FILES
        .iter()
        .map(|name| file_digest(&staging_dir.join(name)).map(|digest| ((*name).to_owned(), digest)))
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let manifest = RuntimeManifest {
        schema: 1,
        versions: plan.versions.clone(),
        digests: plan.digests.clone(),
        file_digests,
    };
    fs::write(
        staging_dir.join(BUNDLE_MANIFEST),
        serde_json::to_vec_pretty(&manifest)?,
    )?;

    if installed_dir.exists() {
        fs::remove_dir_all(&installed_dir)?;
    }
    fs::rename(&staging_dir, &installed_dir)?;
    activate_bundle(&root, &bundle_id)?;

    Ok(plan.versions)
}

pub fn active_yt_dlp_path() -> Option<PathBuf> {
    active_bundle_dir().map(|dir| dir.join("yt-dlp.exe"))
}

fn is_supported_platform() -> bool {
    cfg!(all(target_os = "windows", target_arch = "x86_64"))
}

fn runtime_root() -> Option<PathBuf> {
    dirs::data_local_dir().map(|dir| dir.join("lime-dlp").join("runtime"))
}

fn active_bundle_dir() -> Option<PathBuf> {
    if !is_supported_platform() {
        return None;
    }

    let root = runtime_root()?;
    let bundle_id = fs::read_to_string(root.join("active.txt")).ok()?;
    let bundle_id = bundle_id.trim();
    if bundle_id.len() != 64 || !bundle_id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }

    let dir = root.join("bundles").join(bundle_id);
    bundle_matches(&dir, &read_manifest_digests(&dir)?).then_some(dir)
}

fn bundled_runtime_dir() -> Option<PathBuf> {
    crate::command::bundled_bin_path()?
        .parent()
        .map(Path::to_path_buf)
}

fn bundle_matches(dir: &Path, expected: &RuntimeDigests) -> bool {
    if !runtime_files_present(dir) {
        return false;
    }

    let Some(manifest) = read_manifest(dir) else {
        return false;
    };
    if manifest.schema != 1 || manifest.digests != *expected {
        return false;
    }

    RUNTIME_FILES.iter().all(|name| {
        manifest
            .file_digests
            .get(*name)
            .and_then(|expected| {
                file_digest(&dir.join(name))
                    .ok()
                    .map(|actual| actual == *expected)
            })
            .unwrap_or(false)
    })
}

fn read_manifest(dir: &Path) -> Option<RuntimeManifest> {
    let manifest = fs::read(dir.join(BUNDLE_MANIFEST)).ok()?;
    serde_json::from_slice(&manifest).ok()
}

fn read_manifest_digests(dir: &Path) -> Option<RuntimeDigests> {
    let manifest = read_manifest(dir)?;
    (manifest.schema == 1).then_some(manifest.digests)
}

fn file_digest(path: &Path) -> Result<String, Error> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = std::io::Read::read(&mut file, &mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn runtime_files_present(dir: &Path) -> bool {
    RUNTIME_FILES.iter().all(|name| {
        fs::metadata(dir.join(name))
            .map(|metadata| metadata.is_file() && metadata.len() > 0)
            .unwrap_or(false)
    })
}

fn activate_bundle(root: &Path, bundle_id: &str) -> Result<(), Error> {
    fs::create_dir_all(root)?;
    let next = root.join("active.txt.next");
    fs::write(&next, bundle_id)?;
    fs::rename(next, root.join("active.txt"))?;
    Ok(())
}

fn bundle_id(digests: &RuntimeDigests) -> String {
    let encoded = format!(
        "{}:{}:{}:{}",
        digests.yt_dlp, digests.deno, digests.ffmpeg, digests.ejs
    );
    format!("{:x}", Sha256::digest(encoded.as_bytes()))
}

async fn latest_plan() -> Result<UpdatePlan, Error> {
    let client = api_client()?;
    let yt_dlp_release = fetch_release(&client, "yt-dlp/yt-dlp").await?;
    let deno_release = fetch_release(&client, "denoland/deno").await?;
    let ffmpeg_release = fetch_release(&client, "BtbN/FFmpeg-Builds").await?;
    let ejs_release = fetch_release(&client, "yt-dlp/ejs").await?;

    let yt_dlp = release_asset(&yt_dlp_release, "yt-dlp.exe")?;
    let deno = release_asset(&deno_release, "deno-x86_64-pc-windows-msvc.zip")?;
    let ffmpeg = release_asset(&ffmpeg_release, "ffmpeg-master-latest-win64-gpl.zip")?;
    let ejs = release_asset(&ejs_release, "yt.solver.deno.lib.js")?;

    Ok(UpdatePlan {
        versions: RuntimeVersions {
            yt_dlp: yt_dlp_release.tag_name,
            deno: deno_release.tag_name.trim_start_matches('v').to_owned(),
            ffmpeg: ffmpeg_release.name.unwrap_or(ffmpeg_release.tag_name),
            ejs: ejs_release.tag_name,
        },
        digests: RuntimeDigests {
            yt_dlp: yt_dlp.digest.clone(),
            deno: deno.digest.clone(),
            ffmpeg: ffmpeg.digest.clone(),
            ejs: ejs.digest.clone(),
        },
        yt_dlp,
        deno,
        ffmpeg,
        ejs,
    })
}

fn api_client() -> Result<Client, Error> {
    Client::builder()
        .user_agent(concat!("lime-dlp/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(Error::Request)
}

async fn fetch_release(client: &Client, repository: &str) -> Result<GithubRelease, Error> {
    client
        .get(format!("{GITHUB_API}/{repository}/releases/latest"))
        .send()
        .await?
        .error_for_status()?
        .json::<GithubRelease>()
        .await
        .map_err(Error::Request)
}

fn release_asset(release: &GithubRelease, name: &str) -> Result<DownloadAsset, Error> {
    let asset = release
        .assets
        .iter()
        .find(|asset| asset.name == name)
        .ok_or_else(|| Error::InvalidMetadata(format!("release is missing {name}")))?;
    if !asset
        .browser_download_url
        .starts_with("https://github.com/")
    {
        return Err(Error::InvalidMetadata(format!(
            "unexpected download URL for {name}"
        )));
    }

    let digest = asset
        .digest
        .as_deref()
        .and_then(|digest| digest.strip_prefix("sha256:"))
        .filter(|digest| digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .ok_or_else(|| Error::InvalidMetadata(format!("release has no SHA-256 digest for {name}")))?
        .to_ascii_lowercase();

    Ok(DownloadAsset {
        url: asset.browser_download_url.clone(),
        digest,
    })
}

async fn download_verified(
    client: &Client,
    asset: &DownloadAsset,
    path: &Path,
    max_bytes: u64,
) -> Result<(), Error> {
    let mut response = client.get(&asset.url).send().await?.error_for_status()?;
    if response
        .content_length()
        .is_some_and(|length| length > max_bytes)
    {
        return Err(Error::InvalidMetadata(format!(
            "download is larger than the allowed size: {}",
            asset.url
        )));
    }

    let mut file = tokio::fs::File::create(path).await?;
    let mut hasher = Sha256::new();
    let mut downloaded = 0u64;
    while let Some(chunk) = response.chunk().await? {
        downloaded += chunk.len() as u64;
        if downloaded > max_bytes {
            return Err(Error::InvalidMetadata(format!(
                "download is larger than the allowed size: {}",
                asset.url
            )));
        }
        hasher.update(&chunk);
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await?;
    }
    tokio::io::AsyncWriteExt::flush(&mut file).await?;
    drop(file);

    let actual = format!("{:x}", hasher.finalize());
    if actual != asset.digest {
        let _ = fs::remove_file(path);
        return Err(Error::InvalidMetadata(format!(
            "SHA-256 mismatch for {}",
            asset.url
        )));
    }
    Ok(())
}

fn extract_zip_file(archive_path: &Path, suffix: &str, output_path: &Path) -> Result<(), Error> {
    let mut archive = ZipArchive::new(File::open(archive_path)?)?;
    let mut found = None;
    for index in 0..archive.len() {
        let entry = archive.by_index(index)?;
        let entry_name = entry.name().replace('\\', "/");
        if !entry.is_dir() && entry_name.ends_with(suffix) {
            found = Some(index);
            break;
        }
    }
    let index = found
        .ok_or_else(|| Error::InvalidMetadata(format!("archive does not contain {suffix}")))?;
    let mut entry = archive.by_index(index)?;
    let mut output = File::create(output_path)?;
    io::copy(&mut entry, &mut output)?;
    output.flush()?;
    if fs::metadata(output_path)?.len() == 0 {
        return Err(Error::InvalidMetadata(format!(
            "archive entry {suffix} was empty"
        )));
    }
    Ok(())
}
