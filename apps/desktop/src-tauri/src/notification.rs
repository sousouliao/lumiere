use std::sync::{Mutex, mpsc};
use tauri::{AppHandle, Manager};
use windows::{
    Data::Xml::Dom::XmlDocument,
    Foundation::TypedEventHandler,
    UI::Notifications::{ToastNotification, ToastNotificationManager, ToastNotifier},
    Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize},
    core::{HSTRING, IInspectable},
};

pub struct Notifications(Mutex<Option<Worker>>);
struct Worker {
    sender: mpsc::Sender<Command>,
    thread: std::thread::JoinHandle<()>,
}
enum Command {
    Show(Box<AppHandle>, u64, String, String),
    Clear,
    Stop,
}
impl Notifications {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        let thread = std::thread::spawn(move || {
            // SAFETY: this worker owns one MTA for its entire WinRT lifetime.
            if unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.is_err() {
                return;
            }
            let _apartment = Apartment;
            let mut current = None;
            while let Ok(command) = receiver.recv() {
                current.take();
                match command {
                    Command::Show(app, id, title, detail) => {
                        match FailureNotification::show(&app, id, &title, &detail) {
                            Ok(notification) => current = Some(notification),
                            Err(error) => eprintln!(
                                "{}",
                                serde_json::json!({"level":"warn","event":"capture-notification-unavailable","error":error.to_string()})
                            ),
                        }
                    }
                    Command::Clear => {}
                    Command::Stop => break,
                }
            }
            drop(current);
        });
        Self(Mutex::new(Some(Worker { sender, thread })))
    }
    pub fn restart(&self) {
        let mut slot = self.0.lock().unwrap();
        if slot.is_none() {
            *slot = Self::new().0.lock().unwrap().take();
        }
    }
    pub fn show(&self, app: &AppHandle, id: u64, title: &str, detail: &str) {
        if let Some(worker) = self.0.lock().unwrap().as_ref() {
            let _ = worker.sender.send(Command::Show(
                Box::new(app.clone()),
                id,
                title.into(),
                detail.into(),
            ));
        }
    }
    pub fn clear(&self) {
        if let Some(worker) = self.0.lock().unwrap().as_ref() {
            let _ = worker.sender.send(Command::Clear);
        }
    }
    pub fn shutdown(&self) {
        if let Some(worker) = self.0.lock().unwrap().take() {
            let _ = worker.sender.send(Command::Stop);
            let _ = worker.thread.join();
        }
    }
}
impl Drop for Notifications {
    fn drop(&mut self) {
        self.shutdown();
    }
}

struct FailureNotification {
    notifier: ToastNotifier,
    toast: ToastNotification,
    activated: i64,
}
impl FailureNotification {
    pub fn show(
        app: &AppHandle,
        id: u64,
        title: &str,
        detail: &str,
    ) -> windows::core::Result<Self> {
        let xml = XmlDocument::new()?;
        xml.LoadXml(&HSTRING::from(format!("<toast><visual><binding template=\"ToastGeneric\"><text>{}</text><text>{} Click to open Lumiere.</text></binding></visual><audio silent=\"true\"/></toast>",escape(title),escape(detail))))?;
        let toast = ToastNotification::CreateToastNotification(&xml)?;
        let handle = app.clone();
        let activated = toast.Activated(
            &TypedEventHandler::<ToastNotification, IInspectable>::new(move |_, _| {
                let app = handle.clone();
                tauri::async_runtime::spawn(async move {
                    if app
                        .state::<crate::Controller>()
                        .completion_mode(id)
                        .is_some()
                    {
                        let _ = crate::show_window(&app, false);
                    }
                });
                Ok(())
            }),
        )?;
        let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(
            "io.github.sousouliao.lumiere",
        ))?;
        let notification = Self {
            notifier,
            toast,
            activated,
        };
        notification.notifier.Show(&notification.toast)?;
        Ok(notification)
    }
}
impl Drop for FailureNotification {
    fn drop(&mut self) {
        let _ = self.toast.RemoveActivated(self.activated);
        let _ = self.notifier.Hide(&self.toast);
    }
}
struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) {
        // SAFETY: paired with this scope's successful RoInitialize.
        unsafe { RoUninitialize() }
    }
}
fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
