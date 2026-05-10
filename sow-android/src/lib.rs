//! Android shell: polls lifecycle until destroy. Full Blade/winit client wiring is TODO.

#[cfg(target_os = "android")]
use android_activity::{AndroidApp, MainEvent, PollEvent};
#[cfg(target_os = "android")]
use std::cell::Cell;
#[cfg(target_os = "android")]
use std::time::Duration;

#[cfg(target_os = "android")]
#[no_mangle]
pub fn android_main(app: AndroidApp) {
    android_logger::init_once(android_logger::Config::default().with_max_level(log::LevelFilter::Info));
    log::info!("sow-android: stub activity (game UI not wired yet)");

    loop {
        let quit = Cell::new(false);
        app.poll_events(Some(Duration::from_millis(50)), |event| {
            if let PollEvent::Main(MainEvent::Destroy) = event {
                quit.set(true);
            }
        });
        if quit.get() {
            log::info!("sow-android: destroy");
            break;
        }
    }
}
