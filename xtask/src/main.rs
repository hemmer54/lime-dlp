use anyhow::{anyhow, Context};
use cargo_metadata::MetadataCommand;
use sha2::Digest;
use std::io::{BufRead, Write};
use std::{fs, fs::File, path::Path, process::ExitCode};
use toml_edit::DocumentMut;

use xtask::{cargo, git, unzip, zip_dir, CommandExt};

mod flags {
    xflags::xflags! {
        cmd app {
            cmd package-rpm {}
            cmd package-deb {}
            cmd package-linux {}
            cmd package-linux-all {}
            cmd package-windows {}
            cmd update-windows-assets {}
            cmd package-aur {
                /// Specify rel for PKGBUILD (only works package_aur task).
                optional -r,--rel rel: u8
            }
            cmd publish-aur {
                /// AUR package commit message.
                optional -m,--message message: String
            }
            cmd new-version {
                required version: String
                /// AUR package commit message.
                optional -m,--message message: String
            }
        }
    }
}

fn main() -> ExitCode {
    let flags = flags::App::from_env_or_exit();

    if let Err(e) = std::fs::create_dir_all("packages") {
        eprintln!("Failed to create packages directory: {e}");
        return ExitCode::FAILURE;
    }

    let res = match flags.subcommand {
        flags::AppCmd::PackageRpm(_package_rpm) => crate::package_rpm(),
        flags::AppCmd::PackageDeb(_package_deb) => crate::package_deb(),
        flags::AppCmd::PackageLinux(_package_linux) => crate::package_linux(),
        flags::AppCmd::PackageLinuxAll(_package_linux_all) => crate::package_linux_all(),
        flags::AppCmd::PackageWindows(_package_windows) => crate::package_windows(),
        flags::AppCmd::UpdateWindowsAssets(_update_windows_assets) => {
            crate::update_windows_assets()
        }
        flags::AppCmd::PackageAur(package_aur) => crate::package_aur(package_aur.rel),
        flags::AppCmd::PublishAur(publish_aur) => crate::publish_aur(publish_aur.message),
        flags::AppCmd::NewVersion(new_version) => crate::new_version(new_version),
    };

    if let Err(e) = res {
        eprintln!("{e}");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}

fn package_linux() -> anyhow::Result<()> {
    cargo("build")
        .with_arg("--release")
        .run("Building for linux")?;

    std::fs::copy(
        "target/release/ytdlp-gui",
        "packages/ytdlp-gui-linux-x64-86",
    )?;

    println!("Finished building for linux");

    Ok(())
}

fn package_rpm() -> anyhow::Result<()> {
    package_linux()?;

    cargo("install")
        .with_args(["--locked", "cargo-generate-rpm"])
        .run("Installing cargo-generate-rpm")?;

    cargo("generate-rpm").run("Generating RPM package")?;

    for entry in glob::glob("target/generate-rpm/*.rpm").expect("Failed to read glob pattern") {
        let entry = entry?;
        std::fs::rename(
            entry.to_string_lossy().to_string(),
            format!(
                "packages/{}",
                entry
                    .file_name()
                    .ok_or_else(|| anyhow!("No file name"))?
                    .to_string_lossy()
            ),
        )?;
    }

    println!("Finished generating RPM");
    Ok(())
}

fn package_deb() -> anyhow::Result<()> {
    package_linux()?;

    cargo("install")
        .with_arg("cargo-deb")
        .run("Installing cargo-deb")?;

    cargo("deb").run("Generating DEB package")?;

    std::fs::create_dir_all("packages")?;

    for entry in glob::glob("target/debian/*.deb").expect("Failed to read glob pattern") {
        let entry = entry?;
        std::fs::rename(
            entry.to_string_lossy().to_string(),
            format!(
                "packages/{}",
                entry
                    .file_name()
                    .ok_or_else(|| anyhow!("No file name"))?
                    .to_string_lossy()
            ),
        )?;
    }

    Ok(())
}

fn package_linux_all() -> anyhow::Result<()> {
    package_linux()?;
    package_rpm()?;
    package_deb()?;

    Ok(())
}

fn package_windows() -> anyhow::Result<()> {
    std::fs::create_dir_all("windows")?;
    std::fs::create_dir_all("packages")?;

    if !copy_bundled_windows_runtime()? {
        println!("Downloading ffmpeg");
        let ffmpeg_zip = minreq::get(
            "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip",
        )
        .send()?
        .into_bytes();

        File::create("windows/ffmpeg-master-latest-win64-gpl.zip")?.write_all(&ffmpeg_zip)?;

        println!("Unzipping ffmpeg");
        unzip("windows/ffmpeg-master-latest-win64-gpl.zip", "windows")?;

        let ffmpeg_bin = "windows/ffmpeg-master-latest-win64-gpl/bin";
        for executable in ["ffmpeg.exe", "ffprobe.exe", "ffplay.exe"] {
            let source = format!("{ffmpeg_bin}/{executable}");
            if Path::new(&source).is_file() {
                std::fs::rename(source, format!("windows/{executable}"))?;
            }
        }

        let _ = std::fs::remove_dir_all("windows/ffmpeg-master-latest-win64-gpl");
        let _ = std::fs::remove_file("windows/ffmpeg-master-latest-win64-gpl.zip");

        println!("Downloading yt-dlp");
        let ytdlp =
            minreq::get("https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe")
                .send()?
                .into_bytes();
        File::create("windows/yt-dlp.exe")?.write_all(&ytdlp)?;
        println!("Saved yt-dlp to windows/yt-dlp.exe");
    }

    #[cfg(target_os = "windows")]
    {
        cargo("build")
            .with_arg("--release")
            .run("Building for Windows")?;

        std::fs::rename("target/release/lime-dlp.exe", "windows/lime-dlp.exe")?;
    }

    #[cfg(unix)]
    {
        cargo("build")
            .with_args(["--release", "--target", "x86_64-pc-windows-gnu"])
            .run("Building for Windows")?;

        std::fs::rename(
            "target/x86_64-pc-windows-gnu/release/lime-dlp.exe",
            "windows/lime-dlp.exe",
        )?;
    }

    println!("Zipping Windows package");
    zip_dir("windows", "packages/lime-dlp-windows-64.zip")?;

    Ok(())
}

fn update_windows_assets() -> anyhow::Result<()> {
    const ASSET_DIR: &str = "bin(assets)";
    fs::create_dir_all(ASSET_DIR)?;
    let temp_dir = Path::new(ASSET_DIR).join(".update-temp");
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir)?;
    }
    fs::create_dir_all(&temp_dir)?;

    let ytdlp_url = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe";
    let ytdlp = download(ytdlp_url)?;
    let ytdlp_sums =
        download("https://github.com/yt-dlp/yt-dlp/releases/latest/download/SHA2-256SUMS")?;
    verify_sha256("yt-dlp.exe", &ytdlp, &ytdlp_sums)?;
    fs::write(temp_dir.join("yt-dlp.exe"), ytdlp)?;

    let deno_url =
        "https://github.com/denoland/deno/releases/latest/download/deno-x86_64-pc-windows-msvc.zip";
    let deno_zip = download(deno_url)?;
    let deno_dir = temp_dir.join("deno");
    fs::create_dir_all(&deno_dir)?;
    let deno_zip_path = temp_dir.join("deno.zip");
    fs::write(&deno_zip_path, deno_zip)?;
    unzip(deno_zip_path.to_str().unwrap(), deno_dir.to_str().unwrap())?;
    copy_named_file(&deno_dir, "deno.exe", &temp_dir.join("deno.exe"))?;

    let ffmpeg_url = "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip";
    let ffmpeg_zip = download(ffmpeg_url)?;
    let ffmpeg_sums = download(
        "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/checksums.sha256",
    )?;
    verify_sha256(
        "ffmpeg-master-latest-win64-gpl.zip",
        &ffmpeg_zip,
        &ffmpeg_sums,
    )?;
    let ffmpeg_zip_path = temp_dir.join("ffmpeg.zip");
    fs::write(&ffmpeg_zip_path, ffmpeg_zip)?;
    let ffmpeg_dir = temp_dir.join("ffmpeg");
    fs::create_dir_all(&ffmpeg_dir)?;
    unzip(
        ffmpeg_zip_path.to_str().unwrap(),
        ffmpeg_dir.to_str().unwrap(),
    )?;
    for name in ["ffmpeg.exe", "ffprobe.exe", "ffplay.exe"] {
        copy_named_file(&ffmpeg_dir, name, &temp_dir.join(name))?;
    }

    let ejs: serde_json::Value = serde_json::from_slice(&download(
        "https://api.github.com/repos/yt-dlp/ejs/releases/latest",
    )?)?;
    let ejs_asset = ejs["assets"]
        .as_array()
        .and_then(|assets| {
            assets
                .iter()
                .find(|asset| asset["name"] == "yt.solver.deno.lib.js")
        })
        .context("yt-dlp EJS release did not contain yt.solver.deno.lib.js")?;
    let ejs_bytes = download(
        ejs_asset["browser_download_url"]
            .as_str()
            .context("invalid EJS download URL")?,
    )?;
    let expected = ejs_asset["digest"]
        .as_str()
        .unwrap_or_default()
        .strip_prefix("sha256:")
        .unwrap_or_default();
    verify_digest("yt.solver.deno.lib.js", &ejs_bytes, expected)?;
    fs::write(temp_dir.join("yt.solver.deno.lib.js"), ejs_bytes)?;

    for name in [
        "deno.exe",
        "ffmpeg.exe",
        "ffplay.exe",
        "ffprobe.exe",
        "yt-dlp.exe",
        "yt.solver.deno.lib.js",
    ] {
        fs::copy(temp_dir.join(name), Path::new(ASSET_DIR).join(name))?;
    }
    fs::remove_dir_all(temp_dir)?;
    println!("Updated Windows runtime assets in {ASSET_DIR}");
    Ok(())
}

fn download(url: &str) -> anyhow::Result<Vec<u8>> {
    let response = minreq::get(url)
        .with_header("User-Agent", "lime-dlp-asset-updater/1")
        .send()?;
    if !(200..300).contains(&response.status_code) {
        return Err(anyhow!(
            "download failed with HTTP {}: {url}",
            response.status_code
        ));
    }
    Ok(response.into_bytes())
}

fn verify_sha256(name: &str, bytes: &[u8], sums: &[u8]) -> anyhow::Result<()> {
    let digest = format!("{:x}", sha2::Sha256::digest(bytes));
    let line = String::from_utf8_lossy(sums);
    let expected = line
        .lines()
        .find(|line| {
            line.split_whitespace()
                .last()
                .map(|path| path.ends_with(name))
                .unwrap_or(false)
        })
        .and_then(|line| line.split_whitespace().next())
        .context(format!("checksum for {name} was not found"))?;
    verify_digest(name, bytes, expected)?;
    if digest != expected {
        return Err(anyhow!("checksum mismatch for {name}"));
    }
    Ok(())
}

fn verify_digest(name: &str, bytes: &[u8], expected: &str) -> anyhow::Result<()> {
    let actual = format!("{:x}", sha2::Sha256::digest(bytes));
    if expected.is_empty() || !actual.eq_ignore_ascii_case(expected) {
        return Err(anyhow!(
            "checksum mismatch for {name}: expected {expected}, got {actual}"
        ));
    }
    Ok(())
}

fn copy_named_file(root: &Path, name: &str, destination: &Path) -> anyhow::Result<()> {
    let source = walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .map(|entry| entry.path().to_path_buf())
        .find(|path| path.file_name().and_then(|name| name.to_str()) == Some(name))
        .context(format!("{name} was not found in extracted archive"))?;
    fs::copy(source, destination)?;
    Ok(())
}

fn copy_bundled_windows_runtime() -> anyhow::Result<bool> {
    const FILES: [&str; 6] = [
        "deno.exe",
        "ffmpeg.exe",
        "ffplay.exe",
        "ffprobe.exe",
        "yt-dlp.exe",
        "yt.solver.deno.lib.js",
    ];
    let source_dir = Path::new("bin(assets)");
    if !FILES.iter().all(|name| source_dir.join(name).is_file()) {
        return Ok(false);
    }

    println!("Copying yt-dlp runtime bundle from bin(assets)");
    for name in FILES {
        std::fs::copy(source_dir.join(name), Path::new("windows").join(name))
            .with_context(|| format!("copying Windows runtime asset {name}"))?;
    }
    Ok(true)
}

fn package_aur(rel: Option<u8>) -> anyhow::Result<()> {
    let mut ytdlp_gui_path = std::env::current_dir()?;
    if ytdlp_gui_path.ends_with("xtask") {
        ytdlp_gui_path.pop();
    }
    let metadata = MetadataCommand::new()
        .manifest_path(ytdlp_gui_path.join("Cargo.toml"))
        .exec()
        .unwrap();

    let root_package = metadata.root_package().unwrap();

    let version = root_package.version.to_string();

    let aur_path = ytdlp_gui_path.join("aur");
    let pkgbuild_path = aur_path.join("PKGBUILD");

    let sha256 = {
        let file_name = format!("v{version}.tar.gz");
        let source_code = minreq::get(format!(
            "https://github.com/BKSalman/ytdlp-gui/archive/refs/tags/{}",
            file_name
        ))
        .send()
        .context("failed to get source code from github")?
        .into_bytes();

        println!("Downloaded source code of tag {file_name}");

        let sha = sha2::Sha256::digest(source_code);
        let hex = hex::encode(sha);
        println!("sha256 of source code: {hex}");
        hex
    };

    let pkgbuild = std::fs::read_to_string(&pkgbuild_path)?;

    let pkgbuild = pkgbuild
        .lines()
        .try_fold(String::new(), |mut final_str, line| {
            if line.starts_with("pkgver=") {
                final_str.push_str(&format!("pkgver={}\n", version));
            } else if line.starts_with("sha256sums=") {
                final_str.push_str(&format!("sha256sums=(\"{}\")\n", sha256));
            } else if line.starts_with("pkgrel") {
                if let Some(rel) = rel {
                    final_str.push_str(&format!("pkgrel={}\n", rel));
                } else {
                    final_str.push_str(&format!("{}\n", line));
                }
            } else {
                final_str.push_str(&format!("{}\n", line));
            }

            anyhow::Ok(final_str)
        })?;

    println!("PKGBULID:\n\n{pkgbuild}");

    println!("Do you want to proceed with printing to .SRCINFO? [Y/n]");

    {
        let mut stdin = std::io::stdin().lock();

        let mut buf = String::new();

        stdin.read_line(&mut buf)?;

        if buf.to_lowercase() == "n\n" {
            return Ok(());
        } else if buf.to_lowercase() != "\n" && buf.to_lowercase() != "y\n" {
            return Err(anyhow!("Please enter y or n"));
        }
    }

    std::fs::write(&pkgbuild_path, pkgbuild)?;

    let old_current_dir = std::env::current_dir()?;

    std::env::set_current_dir(&aur_path)?;

    let srcinfo = std::process::Command::new("makepkg")
        .with_args([&pkgbuild_path.display().to_string(), "--printsrc"])
        .run_with_output("Printing to .SRCINFO\n")?;

    let srcinfo = String::from_utf8_lossy(&srcinfo.stdout).to_string();

    println!("srcinfo:\n\n{srcinfo}");

    std::env::set_current_dir(old_current_dir)?;

    let srcinfo_path = aur_path.join(".SRCINFO");

    std::fs::write(srcinfo_path, srcinfo)?;

    Ok(())
}

fn publish_aur(message: Option<String>) -> anyhow::Result<()> {
    let mut ytdlp_gui_path = std::env::current_dir()?;
    if ytdlp_gui_path.ends_with("xtask") {
        ytdlp_gui_path.pop();
    }

    let pkgbuild_path = ytdlp_gui_path.join("aur/PKGBUILD");
    let srcinfo_path = ytdlp_gui_path.join("aur/.SRCINFO");

    let pkgbuild = std::fs::read_to_string(&pkgbuild_path).context("failed to read PKGBUILD")?;

    println!("PKGBUILD:\n\n{pkgbuild}");

    println!("Do you want to proceed with publishing the package? [Y/n]");

    {
        let mut stdin = std::io::stdin().lock();

        let mut buf = String::new();

        stdin.read_line(&mut buf)?;

        if buf.to_lowercase() == "n\n" {
            return Ok(());
        } else if buf.to_lowercase() != "\n" && buf.to_lowercase() != "y\n" {
            return Err(anyhow!("Please enter y or n"));
        }
    }

    let pkgname = pkgbuild
        .lines()
        .find(|l| l.starts_with("pkgname"))
        .map(|p| p.split_once('=').unwrap().1)
        .ok_or(anyhow!("no pkgname"))?;

    let pkgver = pkgbuild
        .lines()
        .find(|l| l.starts_with("pkgver"))
        .map(|p| p.split_once('=').unwrap().1)
        .ok_or(anyhow!("no pkgver"))?;

    let pkgrel = pkgbuild
        .lines()
        .find(|l| l.starts_with("pkgrel"))
        .map(|p| p.split_once('=').unwrap().1)
        .ok_or(anyhow!("no pkgrel"))?;

    std::env::set_current_dir(std::env::temp_dir())?;

    let temp_aur = std::env::temp_dir().join("ytdlp-gui-aur");

    let _ = std::fs::remove_dir_all(&temp_aur);

    let clone_output = git("clone")
        .with_args([
            "-v",
            &format!("ssh://aur@aur.archlinux.org/{pkgname}.git"),
            "ytdlp-gui-aur",
        ])
        .run_with_inherited_output("Clone AUR package")?;
    println!("git clone output:\n{:?}", clone_output);

    println!("Copying PKGBUILD and .SRCINFO to {}", temp_aur.display());
    std::fs::copy(pkgbuild_path, temp_aur.join("PKGBUILD")).context("failed to copy PKGBUILD")?;
    std::fs::copy(srcinfo_path, temp_aur.join(".SRCINFO")).context("failed to copy .SRCINFO")?;

    std::env::set_current_dir(temp_aur)?;

    let add_output = git("add")
        .with_args(["-v", "."])
        .run_with_inherited_output("Add AUR changes")?;
    println!("git add output:\n{:?}", add_output);

    let commit_output = git("commit")
        .with_args([
            "-v",
            "-m",
            &format!(
                "Update to {pkgver}-{pkgrel} {}",
                message.unwrap_or(String::new())
            ),
        ])
        .run_with_inherited_output("Commiting AUR changes")
        .context("failed to commit AUR changes")?;
    println!("git commit output:\n{:?}", commit_output);

    let push_output = git("push").run_with_inherited_output("Pushing to AUR")?;
    println!("git push output:\n{:?}", push_output);

    Ok(())
}

fn new_version(new_version: flags::NewVersion) -> anyhow::Result<()> {
    let mut ytdlp_gui_path = std::env::current_dir()?;
    if ytdlp_gui_path.ends_with("xtask") {
        ytdlp_gui_path.pop();
    }
    let manifest_path = ytdlp_gui_path.join("Cargo.toml");
    let lock_file_path = ytdlp_gui_path.join("Cargo.lock");
    let metadata = MetadataCommand::new()
        .manifest_path(&manifest_path)
        .exec()
        .unwrap();

    let root_package = metadata.root_package().unwrap();

    let old_version = root_package.version.to_string();

    println!("old version: {old_version}");
    println!("Checking if new version is valid...");
    cargo_metadata::semver::Version::parse(&new_version.version)?;
    println!("new version: {}", new_version.version);

    println!("Do you want to proceed with the new version? [Y/n]");

    {
        let mut stdin = std::io::stdin().lock();

        let mut buf = String::new();

        stdin.read_line(&mut buf)?;

        if buf.to_lowercase() == "n\n" {
            return Ok(());
        } else if buf.to_lowercase() != "\n" && buf.to_lowercase() != "y\n" {
            return Err(anyhow!("Please enter y or n"));
        }
    }

    println!("Changing Cargo.toml version to {}...", new_version.version);
    let manifest_edit = std::fs::read_to_string(&manifest_path)?;
    let mut manifest_edit = manifest_edit.parse::<DocumentMut>()?;
    manifest_edit["package"]["version"] = toml_edit::value(&new_version.version);

    std::fs::write(&manifest_path, manifest_edit.to_string())?;
    println!("Changed Cargo.toml version to {} ✅", new_version.version);

    // Update Cargo.lock to reflect the new version
    println!("Updating Cargo.lock...");
    std::env::set_current_dir(&ytdlp_gui_path)?;
    cargo("check").run("Updating Cargo.lock with new version")?;
    println!("Updated Cargo.lock ✅");

    git("diff")
        .with_args([
            manifest_path.display().to_string(),
            lock_file_path.display().to_string(),
        ])
        .run_with_inherited_output("Diff:\n")?;

    println!("Do you want to commit the new diff? [Y/n]");

    {
        let mut stdin = std::io::stdin().lock();

        let mut buf = String::new();

        stdin.read_line(&mut buf)?;

        if buf.to_lowercase() == "n\n" {
            return Ok(());
        } else if buf.to_lowercase() != "\n" && buf.to_lowercase() != "y\n" {
            return Err(anyhow!("Please enter y or n"));
        }
    }

    println!("Pushing new version to Github");

    git("add")
        .with_args([
            manifest_path.display().to_string(),
            lock_file_path.display().to_string(),
        ])
        .run("git add")?;
    git("commit")
        .with_args([
            String::from("-m"),
            format!("bump version to {}", new_version.version),
        ])
        .run("git commit")
        .ok();
    let tag = format!("v{}", new_version.version);
    git("tag")
        .with_arg(&tag)
        .run(&format!("git tag {}", tag))
        .ok();
    git("push").run("git push")?;
    git("push")
        .with_args(["origin", &tag])
        .run(&format!("git push origin {}", tag))?;

    println!("Packaging application for AUR");
    package_aur(Some(1))?;

    println!("Publishing AUR package");
    publish_aur(new_version.message)?;

    Ok(())
}
