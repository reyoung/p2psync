use crate::tracker::AnnounceRequest;
use reqwest;
use std::{sync::Arc, time::Duration};
use tokio::task::JoinHandle;

pub struct HeartBeater {
    handles: Vec<JoinHandle<()>>,
}

impl HeartBeater {
    pub fn new(self_url: &str, trackers: Vec<String>, interval: Duration) -> Box<Self> {
        let client = Arc::new(reqwest::Client::new());

        Box::new(HeartBeater {
            handles: trackers
                .iter()
                .map(|url| (url.clone(), client.clone(), String::from(self_url)))
                .map(move |(mut url, client, self_url)| {
                    tokio::spawn(async move {
                        url.push_str("/announce");
                        loop {
                            let req = AnnounceRequest {
                                addr: self_url.clone(),
                            };

                            if let Err(err) = client.post(url.as_str()).json(&req).send().await {
                                eprintln!("Failed to send heartbeat: {}", err);
                            }

                            tokio::time::sleep(interval).await;
                        }
                    })
                })
                .collect::<Vec<_>>(),
        })
    }

    pub fn stop(&self) {
        for handle in self.handles.iter() {
            handle.abort();
        }
    }
}
