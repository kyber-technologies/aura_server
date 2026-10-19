use crate::error::Error;
use crate::types::FastDashMap;
use crate::types::user::Notification;
use tokio::sync::broadcast;

pub struct NotificationManager {
    channels: FastDashMap<String, broadcast::Sender<Notification>>,
}

impl NotificationManager {
    pub fn new() -> Self {
        Self {
            channels: FastDashMap::default(),
        }
    }

    pub async fn subscribe(
        &self,
        user_id: &str,
    ) -> Result<broadcast::Receiver<Notification>, Error> {
        let sender = self.channels.entry(user_id.to_string()).or_insert_with(|| {
            let (tx, _rx) = broadcast::channel(100);
            tx
        });

        Ok(sender.subscribe())
    }

    pub async fn send(&self, user_id: &str, notification: Notification) {
        if let Some(sender) = self.channels.get(user_id) {
            let _ = sender.send(notification);
        }
    }
}
