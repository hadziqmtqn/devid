//! New-mail notifications (Phase 9, D-050). macOS's UserNotifications framework refuses an ad-hoc
//! signed app ("Notifications are not allowed for this application", tested 2026-11-16), so Devid
//! posts with the older NSUserNotificationCenter. Its first notification makes macOS ask for
//! permission (Options › Allow).

use std::sync::OnceLock;
use tauri::AppHandle;

/// The app, for the delegate's Show.
static APP: OnceLock<AppHandle> = OnceLock::new();

/// Sets the delegate that shows banners while the window is in front and opens the Mail page on
/// Show. Call once, on the main thread (Tauri's setup).
pub fn start(app: &AppHandle) {
    let _ = APP.set(app.clone());
    #[cfg(target_os = "macos")]
    mac::set_delegate();
}

/// Posts one notification, from any thread. Nothing happens outside an app bundle (a debug build
/// run from a terminal), where macOS has no notification center for the process.
pub fn post(app: &AppHandle, title: String, subtitle: String, text: String) {
    #[cfg(target_os = "macos")]
    let _ = app.run_on_main_thread(move || mac::deliver(&title, &subtitle, &text));
    #[cfg(not(target_os = "macos"))]
    let _ = (app, title, subtitle, text);
}

#[cfg(target_os = "macos")]
#[allow(deprecated)] // NSUserNotification, on purpose: see the module doc (D-050).
mod mac {
    use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained, runtime::ProtocolObject};
    use objc2_foundation::{
        NSBundle, NSObject, NSObjectProtocol, NSString, NSUserNotification, NSUserNotificationCenter,
        NSUserNotificationCenterDelegate, NSUserNotificationDefaultSoundName,
    };

    define_class!(
        // SAFETY: NSObject has no subclassing requirements, and Delegate does not implement Drop.
        #[unsafe(super = NSObject)]
        #[thread_kind = MainThreadOnly]
        #[name = "DevidNotificationDelegate"]
        struct Delegate;

        // SAFETY: NSObjectProtocol has no safety requirements.
        unsafe impl NSObjectProtocol for Delegate {}

        // SAFETY: the signatures are the protocol's.
        unsafe impl NSUserNotificationCenterDelegate for Delegate {
            // Banners show while the Devid window is in front, too.
            #[unsafe(method(userNotificationCenter:shouldPresentNotification:))]
            fn should_present(&self, _center: &NSUserNotificationCenter, _notification: &NSUserNotification) -> bool {
                true
            }

            // Show opens the window on the Mail page.
            #[unsafe(method(userNotificationCenter:didActivateNotification:))]
            fn did_activate(&self, _center: &NSUserNotificationCenter, _notification: &NSUserNotification) {
                if let Some(app) = super::APP.get() {
                    crate::open_window(app, Some("mail"));
                }
            }
        }
    );

    /// Without a bundle identifier, the notification center does not exist for the process.
    fn bundled() -> bool {
        NSBundle::mainBundle().bundleIdentifier().is_some()
    }

    pub fn set_delegate() {
        let Some(mtm) = MainThreadMarker::new() else { return };
        if !bundled() {
            return;
        }
        let delegate: Retained<Delegate> = unsafe { msg_send![Delegate::alloc(mtm), init] };
        let center = NSUserNotificationCenter::defaultUserNotificationCenter();
        // SAFETY: the center keeps an unretained reference; the delegate is never freed (forget below).
        unsafe { center.setDelegate(Some(ProtocolObject::from_ref(&*delegate))) };
        std::mem::forget(delegate);
    }

    pub fn deliver(title: &str, subtitle: &str, text: &str) {
        if !bundled() {
            return;
        }
        let n = NSUserNotification::new();
        n.setTitle(Some(&NSString::from_str(title)));
        n.setSubtitle(Some(&NSString::from_str(subtitle)));
        n.setInformativeText(Some(&NSString::from_str(text)));
        // SAFETY: a constant string that Foundation exports.
        n.setSoundName(Some(unsafe { NSUserNotificationDefaultSoundName }));
        NSUserNotificationCenter::defaultUserNotificationCenter().deliverNotification(&n);
    }
}
