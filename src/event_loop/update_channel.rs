use crate::model::block::BlockUpdate;
use tokio::sync::mpsc::{error::SendError, Sender};

#[derive(Clone)]
pub struct UpdateChannel(Sender<BlockUpdate>);

impl From<Sender<BlockUpdate>> for UpdateChannel {
    fn from(ch: Sender<BlockUpdate>) -> Self {
        Self(ch)
    }
}

impl From<&Sender<BlockUpdate>> for UpdateChannel {
    fn from(ch: &Sender<BlockUpdate>) -> Self {
        Self(ch.clone())
    }
}

impl UpdateChannel {
    pub async fn send(&self, mut u: BlockUpdate) -> Result<(), SendError<BlockUpdate>> {
        let new_len = u.as_str().trim_end_matches('\n').len();
        u.text_mut().truncate(new_len);
        if u.as_str().is_empty() {
            return Ok(())
        }
        self.0.send(u).await
    }
}
