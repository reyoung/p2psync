use crate::downloader::planer::Action;
use std::io;

async fn execute_action(action: &Action) -> io::Result<()> {
    Ok(())
}

pub async fn execute_actions(actions: &Vec<Action>, concurrency: usize) -> io::Result<()> {
    Ok(())
}
