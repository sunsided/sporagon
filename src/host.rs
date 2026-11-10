//! The page the browser build runs in. The games site keeps the game's canvas for the whole
//! visit and, when the game is opened again, dispatches [`RESTART`] on it to ask for a fresh
//! game. Natively there is no host and nothing ever asks.

/// `id` of the `<canvas>` the browser build draws into: index.html's, or the one the games
/// site creates for the game.
pub const CANVAS: &str = "sporagon";

/// The `CustomEvent` the host dispatches on [`CANVAS`] to restart the game.
#[cfg(target_arch = "wasm32")]
const RESTART: &str = "sporagon:restart";

/// The scene is painted and the game is about to show its first frame: take down the page's
/// loading notice (`#loading` in index.html), if it has one.
#[cfg(target_arch = "wasm32")]
pub fn loaded() {
    if let Some(notice) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id("loading"))
    {
        notice.remove();
    }
}

/// Natively there is no page to tidy up.
#[cfg(not(target_arch = "wasm32"))]
pub fn loaded() {}

/// Whether the host asked for a new game since the last call. Call it once per frame.
#[cfg(target_arch = "wasm32")]
pub fn restart_requested() -> bool {
    web::listen();
    web::REQUESTED.replace(false)
}

/// Whether the host asked for a new game since the last call: never, natively.
#[cfg(not(target_arch = "wasm32"))]
pub fn restart_requested() -> bool {
    false
}

#[cfg(target_arch = "wasm32")]
mod web {
    use std::cell::Cell;
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    thread_local! {
        /// A restart arrived and has not been taken yet.
        pub(super) static REQUESTED: Cell<bool> = const { Cell::new(false) };
        /// The listener is installed.
        static LISTENING: Cell<bool> = const { Cell::new(false) };
    }

    /// Listen for [`super::RESTART`] on the canvas, once it exists. Tried again each frame
    /// until it does; the canvas is never torn down, so the listener lives as long as the
    /// page.
    pub(super) fn listen() {
        if LISTENING.get() {
            return;
        }
        let Some(canvas) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id(super::CANVAS))
        else {
            return;
        };
        let on_restart = Closure::<dyn FnMut(web_sys::Event)>::new(|_: web_sys::Event| {
            REQUESTED.set(true);
        });
        if canvas
            .add_event_listener_with_callback(super::RESTART, on_restart.as_ref().unchecked_ref())
            .is_ok()
        {
            on_restart.forget();
            LISTENING.set(true);
        }
    }
}
