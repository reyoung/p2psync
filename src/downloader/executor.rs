use tokio::{fs, io::AsyncWriteExt};

use crate::downloader::planer::Action;
use std::sync::{Arc, Mutex};
use std::{collections::VecDeque, io, path::Path};

struct DownloadURLs<'a> {
    md5: &'a str,
    peers: &'a Vec<String>,
    peer_id: usize,
    cur: usize,
}

impl<'a> Iterator for DownloadURLs<'a> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cur >= self.peers.len() {
            None
        } else {
            let offset = (self.peer_id + self.cur) % self.peers.len();
            self.cur += 1;
            Some(format!("{}/download?md5={}", self.peers[offset], self.md5))
        }
    }
}

async fn download_and_check(
    url: &str,
    md5: &str,
    file_path: &Path,
    pbar: Arc<Mutex<tqdm::Tqdm<()>>>,
) -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
    let mut resp = reqwest::get(url).await?;
    let mut output_file = fs::File::create(file_path).await?;
    let mut md5_context = md5::Context::new();

    while let Some(chunk) = resp.chunk().await? {
        output_file.write_all(&chunk).await?;
        md5_context.consume(&chunk);

        pbar.lock().unwrap().update(chunk.len())?;
    }

    if md5 != format!("{:x}", md5_context.compute()) {
        Err("md5 mismatch".into())
    } else {
        Ok(())
    }
}

async fn execute_action(
    action: Action,
    pbar: Arc<Mutex<tqdm::Tqdm<()>>>,
) -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
    Ok(match action {
        Action::Download {
            peers,
            peer_id,
            path: file_path,
            md5,
            size,
        } => {
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent).await?;
            }

            // Extract peers data before async operations
            let peers_vec = {
                let peers_read_guard = peers
                    .read()
                    .map_err(|err| io::Error::new(io::ErrorKind::Other, err.to_string()))?;
                peers_read_guard.clone()
            };

            let urls = DownloadURLs {
                md5: &md5,
                peers: &peers_vec,
                peer_id,
                cur: 0,
            };

            let mut errs = Vec::new();
            for url in urls {
                match download_and_check(url.as_str(), &md5, file_path.as_path(), pbar.clone())
                    .await
                {
                    Ok(()) => {
                        return Ok(());
                    }
                    Err(err) => errs.push(err),
                }
            }

            let error_msgs: Vec<String> = errs.iter().map(|e| e.to_string()).collect();
            return Err(format!("All download attempts failed: {}", error_msgs.join("; ")).into());
        }
        Action::MakeDir { path } => fs::create_dir_all(path).await?,
    })
}

fn total_size(actions: &Vec<Action>) -> usize {
    let mut total_size = 0;
    for action in actions {
        match action {
            Action::Download { size, .. } => total_size += size,
            Action::MakeDir { .. } => {}
        }
    }
    total_size
}

pub async fn execute_actions(
    actions: &Vec<Action>,
    concurrency: usize,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut handles = VecDeque::new();
    let mut errs = Vec::new();

    let tqdm = Arc::new(Mutex::new(
        tqdm::pbar(Some(total_size(actions))).desc(Some("download")),
    ));
    for action in actions {
        handles.push_back(tokio::spawn(execute_action(action.clone(), tqdm.clone())));

        if handles.len() > concurrency {
            let handle = handles.pop_front().unwrap();
            if let Err(err) = handle.await {
                errs.push(err);
            }
        }

        if !errs.is_empty() {
            break;
        }
    }

    while let Some(handle) = handles.pop_front() {
        if let Err(err) = handle.await {
            errs.push(err);
        }
    }

    if !errs.is_empty() {
        let error_msgs: Vec<String> = errs.iter().map(|e| e.to_string()).collect();
        return Err(format!("Some tasks failed: {}", error_msgs.join("; ")).into());
    }

    Ok(())
}
