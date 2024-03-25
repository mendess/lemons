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
    pub async fn send(&self, u: BlockUpdate) -> Result<(), SendError<BlockUpdate>> {
        self.0.send(u).await
    }
}
