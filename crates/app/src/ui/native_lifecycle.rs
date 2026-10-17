//! Dock reopen and Quit Apple events, without replacing winit's app delegate.

use std::cell::Cell;

use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSWindowDidEnterFullScreenNotification, NSWindowDidExitFullScreenNotification,
    NSWindowWillEnterFullScreenNotification, NSWindowWillExitFullScreenNotification,
};
use objc2_foundation::{
    NSAppleEventDescriptor, NSAppleEventManager, NSNotification, NSNotificationCenter, NSObject,
    NSObjectProtocol,
};
use plotium_core::state::PlotiumApp;

// Core Apple event FourCharCodes from AERegistry.h.
const CORE_EVENT_CLASS: u32 = u32::from_be_bytes(*b"aevt");
const REOPEN_APPLICATION: u32 = u32::from_be_bytes(*b"rapp");
const QUIT_APPLICATION: u32 = u32::from_be_bytes(*b"quit");

struct EventState {
    ctx: egui::Context,
    quit_requested: Cell<bool>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements. AppKit calls these
    // handlers on the main thread; their state is never accessed off-thread.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = EventState]
    struct LifecycleHandler;

    unsafe impl NSObjectProtocol for LifecycleHandler {}

    impl LifecycleHandler {
        #[unsafe(method(fullscreenWillChange:))]
        fn fullscreen_will_change(&self, _notification: &NSNotification) {
            super::window_lifecycle::fullscreen_notification(&self.ivars().ctx, true, false);
        }

        #[unsafe(method(fullscreenDidEnter:))]
        fn fullscreen_did_enter(&self, _notification: &NSNotification) {
            super::window_lifecycle::fullscreen_notification(&self.ivars().ctx, false, false);
        }

        #[unsafe(method(fullscreenDidExit:))]
        fn fullscreen_did_exit(&self, _notification: &NSNotification) {
            super::window_lifecycle::fullscreen_notification(&self.ivars().ctx, false, true);
        }

        #[unsafe(method(handleReopen:withReplyEvent:))]
        fn reopen(&self, _event: &NSAppleEventDescriptor, _reply: &NSAppleEventDescriptor) {
            show_window(&self.ivars().ctx);
        }

        #[unsafe(method(handleQuit:withReplyEvent:))]
        fn quit(&self, _event: &NSAppleEventDescriptor, _reply: &NSAppleEventDescriptor) {
            self.ivars().quit_requested.set(true);
            // An unsaved-changes prompt must be visible even after closing
            // the window. App::logic consumes the request on the next wake.
            show_window(&self.ivars().ctx);
        }
    }
);

pub(super) struct AppleEvents {
    handler: Retained<LifecycleHandler>,
}

impl AppleEvents {
    pub(super) fn new(ctx: &egui::Context) -> Self {
        let mtm = MainThreadMarker::new().expect("native menus are created on the main thread");
        let handler = LifecycleHandler::alloc(mtm).set_ivars(EventState {
            ctx: ctx.clone(),
            quit_requested: Cell::new(false),
        });
        // SAFETY: NSObject's init has this signature and initializes our superclass.
        let handler: Retained<LifecycleHandler> = unsafe { msg_send![super(handler), init] };
        let manager = NSAppleEventManager::sharedAppleEventManager();
        // SAFETY: Both selectors above have Apple's documented handler signature
        // (two NSAppleEventDescriptor arguments, no return value). Event class
        // and ID are 32-bit FourCharCodes. We retain the handler until removal.
        // Use this selector directly to avoid pulling in all of CoreServices
        // only for its two u32 type aliases.
        unsafe {
            let _: () = msg_send![&manager,
                setEventHandler: &*handler,
                andSelector: sel!(handleReopen:withReplyEvent:),
                forEventClass: CORE_EVENT_CLASS,
                andEventID: REOPEN_APPLICATION
            ];
            let _: () = msg_send![&manager,
                setEventHandler: &*handler,
                andSelector: sel!(handleQuit:withReplyEvent:),
                forEventClass: CORE_EVENT_CLASS,
                andEventID: QUIT_APPLICATION
            ];
        }
        let center = NSNotificationCenter::defaultCenter();
        // SAFETY: These selectors each take one NSNotification. Only the root
        // window can enter full screen; tool windows are embedded egui panels.
        // Observe completion without replacing winit's native window delegate.
        unsafe {
            for (name, selector) in [
                (
                    NSWindowWillEnterFullScreenNotification,
                    sel!(fullscreenWillChange:),
                ),
                (
                    NSWindowWillExitFullScreenNotification,
                    sel!(fullscreenWillChange:),
                ),
                (
                    NSWindowDidEnterFullScreenNotification,
                    sel!(fullscreenDidEnter:),
                ),
                (
                    NSWindowDidExitFullScreenNotification,
                    sel!(fullscreenDidExit:),
                ),
            ] {
                center.addObserver_selector_name_object(&handler, selector, Some(name), None);
            }
        }
        Self { handler }
    }

    pub(super) fn poll(&self, app: &mut PlotiumApp) {
        if self.handler.ivars().quit_requested.replace(false) {
            super::commands::execute_without_clipboard(
                super::commands::CommandId::Quit,
                app,
                &self.handler.ivars().ctx,
            );
        }
    }
}

impl Drop for AppleEvents {
    fn drop(&mut self) {
        let manager = NSAppleEventManager::sharedAppleEventManager();
        // SAFETY: Same selector argument types as installation; remove callbacks
        // before releasing the object that receives them.
        unsafe {
            NSNotificationCenter::defaultCenter().removeObserver(&self.handler);
            for id in [REOPEN_APPLICATION, QUIT_APPLICATION] {
                let _: () = msg_send![&manager,
                    removeEventHandlerForEventClass: CORE_EVENT_CLASS,
                    andEventID: id
                ];
            }
        }
    }
}

pub(super) fn show_window(ctx: &egui::Context) {
    super::window_lifecycle::cancel_close(ctx);
    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
    ctx.request_repaint();
}
