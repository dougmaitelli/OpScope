use async_trait::async_trait;
use opscope_core::application::{Notification, NotificationDeliveryFailure, NotificationSink};
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

pub(crate) struct DesktopNotificationSink {
    app: AppHandle,
}

impl DesktopNotificationSink {
    pub(crate) fn new(app: AppHandle) -> Self {
        Self {
            app,
        }
    }
}

#[async_trait]
impl NotificationSink for DesktopNotificationSink {
    async fn send(&self, notification: &Notification) -> Result<(), NotificationDeliveryFailure> {
        self.app
            .notification()
            .builder()
            .title(&notification.title)
            .body(&notification.body)
            .show()
            .map_err(|_| NotificationDeliveryFailure)
    }
}
