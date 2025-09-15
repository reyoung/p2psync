use tokio::fs;

use crate::{downloader::planer::Action, utils};
use std::{io, path::Path};

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

async fn download_and_check(url: &str, md5: &str, file_path: &Path) -> io::Result<()> {
    Ok(())
}

async fn execute_action<'a>(
    action: &'a Action,
) -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
    Ok(match action {
        Action::Download {
            peers,
            peer_id,
            path: file_path,
            md5,
        } => {
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent).await?;
            }
            let peers_read_guard = peers
                .read()
                .map_err(|err| io::Error::new(io::ErrorKind::Other, err.to_string()))?;

            let urls = DownloadURLs {
                md5: md5,
                peers: peers_read_guard.as_ref(),
                peer_id: peer_id.clone(),
                cur: 0,
            };

            let mut errs = Vec::new();
            for url in urls {
                match download_and_check(url.as_str(), md5, file_path.as_path()).await {
                    Ok(()) => {
                        return Ok(());
                    }
                    Err(err) => errs.push(err),
                }
            }

            return Err(Box::new(utils::multierr::MultiError::new(errs)));
        }
        Action::MakeDir { path } => fs::create_dir_all(path).await?,
    })
}

pub async fn execute_actions<'a>(
    actions: &'a Vec<Action>,
    concurrency: usize,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let spawner = utils::limited_spawner::LimitedSpawner::new(concurrency);
    let mut handles = Vec::new();
    for i in 0..actions.len() {
        let handle = spawner
            .spawn(async move { execute_action(&actions[i]).await })
            .await?;
        handles.push(handle);
    }

    for handle in handles {
        handle.await?;
    }

    Ok(())
}
