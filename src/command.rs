use shared_child::SharedChild;
use std::{
    io::{BufRead, BufReader, Read},
    path::PathBuf,
    process::Stdio,
    sync::Arc,
};

use tokio::sync::mpsc::UnboundedSender;

use crate::error::DownloadError;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Default)]
pub struct Command {
    pub shared_child: Option<Arc<SharedChild>>,
    videos_num: usize,
}

fn bin_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    }
}

fn existing_file(path: PathBuf) -> Option<PathBuf> {
    path.is_file().then_some(path)
}

pub fn bundled_bin_path() -> Option<PathBuf> {
    let mut directories = Vec::new();

    if let Ok(executable) = std::env::current_exe()
        && let Some(executable_dir) = executable.parent()
    {
        directories.push(executable_dir.to_path_buf());
        directories.push(executable_dir.join("assets"));
        directories.push(executable_dir.join("bin(assets)"));
        directories.extend(
            executable_dir
                .ancestors()
                .map(|dir| dir.join("bin(assets)")),
        );
    }

    if let Ok(current_dir) = std::env::current_dir() {
        directories.push(current_dir.clone());
        directories.push(current_dir.join("assets"));
        directories.push(current_dir.join("bin(assets)"));
    }

    directories
        .into_iter()
        .map(|directory| directory.join(bin_name()))
        .find_map(existing_file)
}

pub fn resolve_bin_path(configured_path: Option<PathBuf>) -> PathBuf {
    if let Some(ref path) = configured_path {
        if path.is_file() {
            return path.clone();
        }
    }

    if let Some(path) = crate::runtime_update::active_yt_dlp_path()
        .and_then(existing_file)
    {
        return path;
    }

    if let Some(path) = bundled_bin_path() {
        return path;
    }

    if let Some(paths) = std::env::var_os("PATH") {
        for path in std::env::split_paths(&paths) {
            if let Some(candidate) = existing_file(path.join(bin_name())) {
                return candidate;
            }
        }
    }

    configured_path.unwrap_or_else(|| PathBuf::from(bin_name()))
}

impl Command {
    pub fn is_multiple_videos(&self) -> bool {
        self.videos_num > 1
    }

    pub fn finished_single_video(&mut self) {
        self.videos_num -= 1;
    }

    pub fn kill(&mut self) {
        if let Some(child) = self.shared_child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    pub fn start(
        &mut self,
        mut args: Vec<&str>,
        bin_path: Option<PathBuf>,
        sender: UnboundedSender<crate::Message>,
        videos_num: usize,
        _show_console: bool,
    ) -> Option<Result<String, DownloadError>> {
        self.kill();

        self.videos_num = videos_num;

        let actual_bin_path = resolve_bin_path(bin_path);
        let mut command = std::process::Command::new(&actual_bin_path);
        if let Some(binary_dir) = actual_bin_path.parent() {
            if let Some(path) = std::env::var_os("PATH") {
                let mut paths = std::env::split_paths(&path).collect::<Vec<_>>();
                paths.insert(0, binary_dir.to_path_buf());
                if let Ok(path) = std::env::join_paths(paths) {
                    command.env("PATH", path);
                }
            }
        }

        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(CREATE_NO_WINDOW);
        }

        let print = [
            "--print",
            r#"before_dl:__{"type": "pre_download", "video_id": "%(id)s"}"#,
            "--print",
            r#"playlist:__{"type": "end_of_playlist"}"#,
            "--print",
            r#"after_video:__{"type": "end_of_video"}"#,
        ];

        let template = concat!(
            r#"__{"type": "downloading","#,
            r#""eta": %(progress.eta)s, "downloaded_bytes": %(progress.downloaded_bytes)s,"#,
            r#""total_bytes": %(progress.total_bytes)s, "total_bytes_estimate": %(progress.total_bytes_estimate)s,"#,
            r#""elapsed": %(progress.elapsed)s, "speed": %(progress.speed)s, "playlist_count": %(info.playlist_count)s,"#,
            r#""playlist_index": %(info.playlist_index)s }"#
        );

        let progess_template = ["--progress-template", template];

        args.extend_from_slice(&print);
        args.extend_from_slice(&progess_template);
        args.push("--no-quiet");

        let Ok(shared_child) = SharedChild::spawn(
            command
                .args(args)
                .stderr(Stdio::piped())
                .stdout(Stdio::piped()),
        ) else {
            return Some(Err(DownloadError::YtDlpMissing));
        };

        self.shared_child = Some(Arc::new(shared_child));

        let Some(child) = self.shared_child.clone() else {
            return Some(Err(DownloadError::Other));
        };

        if let Some(stderr) = child.take_stderr() {
            let sender = sender.clone();
            std::thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines().map_while(Result::ok) {
                    let _ = sender.send(crate::Message::ProgressEvent(format!("stderr:{line}")));
                }
            });
        }

        if let Some(stdout) = child.take_stdout() {
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stdout);
                let mut buffer = Vec::new();
                let mut byte = [0u8; 1];

                loop {
                    match reader.read(&mut byte) {
                        Ok(0) => break,
                        Ok(_) => {
                            if byte[0] == b'\r' || byte[0] == b'\n' {
                                if !buffer.is_empty() {
                                    let text = String::from_utf8_lossy(&buffer).to_string();
                                    let _ = sender.send(crate::Message::ProgressEvent(text));
                                    buffer.clear();
                                }
                            } else {
                                buffer.push(byte[0]);
                            }
                        }
                        Err(_) => break,
                    }
                }
            });
        }

        Some(Ok(String::from("Initializing...")))
    }

    pub fn is_running(&self) -> bool {
        self.shared_child.is_some()
    }
}
