//! Native AccessKit lifetime and thread handoff. Callbacks only clone committed
//! snapshots or queue requests; they never borrow the runtime or edit controls.
use crate::PlatformError;
use accesskit::{ActionHandler, ActionRequest, ActivationHandler, DeactivationHandler, TreeUpdate};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use view_core::Revision;
use winit::{event::WindowEvent, event_loop::ActiveEventLoop, window::Window};

/// Request tagged with the semantic revision observed by the native callback.
pub struct QueuedAction {
    /// Reject if the application has since committed different semantic state.
    pub revision: Revision,
    /// Action still requires validation against live capabilities and ownership.
    pub request: ActionRequest,
}
type Snapshot = Arc<Mutex<(Revision, TreeUpdate)>>;
struct Activation(Snapshot);
impl ActivationHandler for Activation {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        self.0.lock().ok().map(|s| s.1.clone())
    }
}
struct Actions {
    snapshot: Snapshot,
    sender: mpsc::SyncSender<QueuedAction>,
    overflow: Arc<AtomicBool>,
    window: Arc<Window>,
}
impl ActionHandler for Actions {
    fn do_action(&mut self, request: ActionRequest) {
        if let Ok(snapshot) = self.snapshot.lock() {
            if self
                .sender
                .try_send(QueuedAction {
                    revision: snapshot.0,
                    request,
                })
                .is_err()
            {
                self.overflow.store(true, Ordering::Release);
            }
            self.window.request_redraw();
        }
    }
}
struct Deactivation;
impl DeactivationHandler for Deactivation {
    fn deactivate_accessibility(&mut self) {}
}
/// One adapter per initially hidden window, owned and drained on the UI thread.
pub struct NativeAccessibility {
    adapter: accesskit_winit::Adapter,
    snapshot: Snapshot,
    receiver: mpsc::Receiver<QueuedAction>,
    overflow: Arc<AtomicBool>,
    _thread: std::marker::PhantomData<std::rc::Rc<()>>,
}
impl NativeAccessibility {
    /// Install before the window has ever been shown. Pass a complete tree.
    pub fn new(
        event_loop: &ActiveEventLoop,
        window: Arc<Window>,
        revision: Revision,
        tree: TreeUpdate,
    ) -> Result<Self, PlatformError> {
        if window.is_visible() == Some(true) {
            return Err(PlatformError::WindowCreation(
                "accessibility must be installed before showing the window".into(),
            ));
        }
        let snapshot = Arc::new(Mutex::new((revision, tree)));
        let (sender, receiver) = mpsc::sync_channel(1024);
        let overflow = Arc::new(AtomicBool::new(false));
        let actions = Actions {
            snapshot: snapshot.clone(),
            sender,
            overflow: overflow.clone(),
            window: window.clone(),
        };
        let adapter = accesskit_winit::Adapter::with_direct_handlers(
            event_loop,
            &window,
            Activation(snapshot.clone()),
            actions,
            Deactivation,
        );
        Ok(Self {
            adapter,
            snapshot,
            receiver,
            overflow,
            _thread: std::marker::PhantomData,
        })
    }
    /// Forward every native event before application handling.
    pub fn process_event(&mut self, window: &Window, event: &WindowEvent) {
        self.adapter.process_event(window, event);
    }
    /// Publish a complete coherent tree even while no accessibility client is active.
    pub fn publish(&mut self, revision: Revision, tree: TreeUpdate) -> Result<(), PlatformError> {
        {
            let mut snapshot = self
                .snapshot
                .lock()
                .map_err(|_| PlatformError::EventLoop("accessibility snapshot poisoned".into()))?;
            if revision <= snapshot.0 {
                return Err(PlatformError::EventLoop(
                    "stale accessibility revision".into(),
                ));
            }
            *snapshot = (revision, tree.clone());
        }
        self.adapter.update_if_active(|| tree);
        Ok(())
    }
    /// Drain pending requests. Overflow is explicit rather than silent loss.
    pub fn drain(&mut self) -> Result<Vec<QueuedAction>, PlatformError> {
        if self.overflow.swap(false, Ordering::AcqRel) {
            return Err(PlatformError::EventLoop(
                "accessibility action queue overflow".into(),
            ));
        }
        Ok(self.receiver.try_iter().collect())
    }
}
