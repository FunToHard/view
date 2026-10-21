use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use view_automation::{desktop::*, *};
struct Mock {
    foreground: Arc<AtomicBool>,
    events: Arc<Mutex<Vec<Event>>>,
}
impl Driver for Mock {
    fn check_target(&self) -> Result<(), Error> {
        if self.foreground.load(Ordering::Acquire) {
            Ok(())
        } else {
            Err(Error::new(Code::Cancelled, "focus lost"))
        }
    }
    fn already_down(&self, key: Held) -> bool {
        key == Held::Key(99)
    }
    fn insert(&mut self, event: Event) -> Result<(), Error> {
        self.events.lock().unwrap().push(event);
        Ok(())
    }
}
#[test]
fn passive_mode_focus_loss_timeout_and_stop_release_only_owned_inputs() {
    let foreground = Arc::new(AtomicBool::new(true));
    let events = Arc::new(Mutex::new(vec![]));
    let make = || Mock {
        foreground: foreground.clone(),
        events: events.clone(),
    };
    assert!(DesktopSession::start(make(), false, Duration::from_secs(1)).is_err());
    assert!(events.lock().unwrap().is_empty());
    let session = DesktopSession::start(make(), true, Duration::from_secs(1)).unwrap();
    assert!(DesktopSession::start(make(), true, Duration::from_secs(1)).is_err());
    session.send(Event::Down(Held::Key(16))).unwrap();
    foreground.store(false, Ordering::Release);
    std::thread::sleep(Duration::from_millis(60));
    assert!(session.send(Event::Down(Held::Key(65))).is_err());
    let _ = session.finish();
    assert_eq!(
        *events.lock().unwrap(),
        vec![Event::Down(Held::Key(16)), Event::Up(Held::Key(16))]
    );
    foreground.store(true, Ordering::Release);
    events.lock().unwrap().clear();
    let session = DesktopSession::start(make(), true, Duration::from_secs(1)).unwrap();
    assert!(session.send(Event::Down(Held::Key(99))).is_err());
    let _ = session.finish();
    assert!(events.lock().unwrap().is_empty());
    let session = DesktopSession::start(make(), true, Duration::from_millis(40)).unwrap();
    session.send(Event::Down(Held::Button(0))).unwrap();
    std::thread::sleep(Duration::from_millis(80));
    assert!(session.finish().is_err());
    assert_eq!(
        events.lock().unwrap().last(),
        Some(&Event::Up(Held::Button(0)))
    );
}
#[test]
fn fractional_dpi_negative_monitor_and_layout_races_are_independently_expected() {
    let map = ScreenMap {
        origin: (-1920, 100),
        scale: 1.5,
        revision: 4,
    };
    assert_eq!(
        map.convert(view_core::LogicalPoint::new(20., 40.), 4)
            .unwrap(),
        (-1890, 160)
    );
    assert_eq!(
        map.convert(view_core::LogicalPoint::ZERO, 5)
            .unwrap_err()
            .code,
        Code::Stale
    );
}
