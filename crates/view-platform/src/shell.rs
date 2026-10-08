//! Multi-window ownership, modal hierarchies, close interception and restoration snapshots.

use std::collections::HashMap;
use view_core::{LogicalSize, PhysicalSize, ScaleFactor, WindowId};
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{WindowAttributes, WindowId as WinitWindowId};

use crate::{
    error::PlatformError,
    events::{InputStateTracker, PlatformWindowEvent, translate_window_event},
    window::{PlatformWindow, WindowLifecycle},
};

/// A snapshot of native window geometry and state, enabling view restoration
/// while keeping application documents strictly decoupled.
#[derive(Clone, Debug, PartialEq)]
pub struct WindowStateSnapshot {
    /// Framework window identity.
    pub id: WindowId,
    /// Native window title.
    pub title: String,
    /// Client area logical size.
    pub logical_size: LogicalSize,
    /// Client area physical size.
    pub physical_size: PhysicalSize,
    /// Screen origin coordinates (supports negative virtual desktop space).
    pub client_screen_origin: (i32, i32),
    /// Active scale factor.
    pub scale_factor: ScaleFactor,
    /// Text scale multiplier.
    pub text_scale: f32,
    /// Associated modal parent, if this window was opened as a modal dialog.
    pub modal_parent: Option<WindowId>,
    /// Child modal windows blocking this window.
    pub modal_children: Vec<WindowId>,
    /// Lifecycle state at time of snapshot.
    pub lifecycle: WindowLifecycle,
}

/// The platform shell managing multi-window ownership, modal routing, and native window lifecycle.
pub struct PlatformShell {
    windows: HashMap<WindowId, PlatformWindow>,
    winit_to_core: HashMap<WinitWindowId, WindowId>,
    input_trackers: HashMap<WindowId, InputStateTracker>,
    window_titles: HashMap<WindowId, String>,
}

impl Default for PlatformShell {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformShell {
    /// Create a new, empty platform shell.
    pub fn new() -> Self {
        Self {
            windows: HashMap::new(),
            winit_to_core: HashMap::new(),
            input_trackers: HashMap::new(),
            window_titles: HashMap::new(),
        }
    }

    /// Create and register a native window in the shell with the specified framework ID.
    pub fn create_window(
        &mut self,
        event_loop: &ActiveEventLoop,
        id: WindowId,
        attributes: WindowAttributes,
    ) -> Result<&mut PlatformWindow, PlatformError> {
        let title = attributes.title.clone();
        let winit_window = event_loop
            .create_window(attributes)
            .map_err(|e| PlatformError::WindowCreation(e.to_string()))?;

        let winit_id = winit_window.id();
        let window = PlatformWindow::new(id, winit_window, 1.0)?;

        self.windows.insert(id, window);
        self.winit_to_core.insert(winit_id, id);
        self.input_trackers.insert(id, InputStateTracker::new());
        self.window_titles.insert(id, title);

        Ok(self.windows.get_mut(&id).expect("window just inserted"))
    }

    /// Number of active native windows in the shell.
    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    /// True if no windows are registered.
    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }

    /// List all registered framework window IDs.
    pub fn window_ids(&self) -> Vec<WindowId> {
        self.windows.keys().copied().collect()
    }

    /// Look up a framework WindowId from a native winit WindowId.
    pub fn lookup_id(&self, winit_id: WinitWindowId) -> Option<WindowId> {
        self.winit_to_core.get(&winit_id).copied()
    }

    /// Borrow a window by framework ID.
    pub fn get_window(&self, id: WindowId) -> Option<&PlatformWindow> {
        self.windows.get(&id)
    }

    /// Mutably borrow a window by framework ID.
    pub fn get_window_mut(&mut self, id: WindowId) -> Option<&mut PlatformWindow> {
        self.windows.get_mut(&id)
    }

    /// Borrow a window by winit ID.
    pub fn get_window_by_winit(&self, winit_id: WinitWindowId) -> Option<&PlatformWindow> {
        let id = self.lookup_id(winit_id)?;
        self.get_window(id)
    }

    /// Configure a window as a modal child of another window.
    ///
    /// Validates existence and prevents cycles in the modal hierarchy.
    pub fn set_modal(
        &mut self,
        child_id: WindowId,
        parent_id: WindowId,
    ) -> Result<(), PlatformError> {
        if child_id == parent_id {
            return Err(PlatformError::ModalViolation(
                "A window cannot be modal to itself".into(),
            ));
        }

        if !self.windows.contains_key(&child_id) {
            return Err(PlatformError::InvalidWindow(child_id));
        }
        if !self.windows.contains_key(&parent_id) {
            return Err(PlatformError::InvalidWindow(parent_id));
        }

        // Cycle check: ensure child_id is not an ancestor of parent_id
        let mut curr = Some(parent_id);
        while let Some(ancestor) = curr {
            if ancestor == child_id {
                return Err(PlatformError::ModalViolation(
                    "Cycle detected in modal window hierarchy".into(),
                ));
            }
            curr = self.windows.get(&ancestor).and_then(|w| w.modal_parent());
        }

        // Remove previous modal relationship if child already had a parent
        let old_parent = self.windows.get(&child_id).and_then(|w| w.modal_parent());
        if let Some(old_p) = old_parent
            && let Some(p_win) = self.windows.get_mut(&old_p)
        {
            p_win.remove_modal_child(child_id);
        }

        // Apply new relationship
        if let Some(child_win) = self.windows.get_mut(&child_id) {
            child_win.set_modal_parent(Some(parent_id));
        }
        if let Some(parent_win) = self.windows.get_mut(&parent_id) {
            parent_win.add_modal_child(child_id);
        }

        Ok(())
    }

    /// Remove modal relationship for a window, freeing its parent.
    pub fn remove_modal(&mut self, child_id: WindowId) -> Result<(), PlatformError> {
        let old_parent = self
            .windows
            .get(&child_id)
            .ok_or(PlatformError::InvalidWindow(child_id))?
            .modal_parent();

        if let Some(p_id) = old_parent {
            if let Some(p_win) = self.windows.get_mut(&p_id) {
                p_win.remove_modal_child(child_id);
            }
            if let Some(c_win) = self.windows.get_mut(&child_id) {
                c_win.set_modal_parent(None);
            }
        }

        Ok(())
    }

    /// Check if a window is blocked from receiving input by an active child modal.
    pub fn is_blocked_by_modal(&self, id: WindowId) -> bool {
        self.windows
            .get(&id)
            .map(|w| w.is_blocked_by_modal())
            .unwrap_or(false)
    }

    /// Capture a snapshot of a window's state.
    pub fn snapshot_window(&self, id: WindowId) -> Option<WindowStateSnapshot> {
        let win = self.windows.get(&id)?;
        let title = self.window_titles.get(&id).cloned().unwrap_or_default();
        let dpi = win.dpi();

        Some(WindowStateSnapshot {
            id,
            title,
            logical_size: win.logical_size(),
            physical_size: win.physical_size(),
            client_screen_origin: dpi.client_screen_origin,
            scale_factor: dpi.scale_factor,
            text_scale: dpi.text_scale,
            modal_parent: win.modal_parent(),
            modal_children: win.modal_children().to_vec(),
            lifecycle: win.lifecycle(),
        })
    }

    /// Snapshot all registered windows.
    pub fn snapshot_all(&self) -> Vec<WindowStateSnapshot> {
        self.windows
            .keys()
            .filter_map(|&id| self.snapshot_window(id))
            .collect()
    }

    /// Mark a window as having received a close request from the user/OS.
    ///
    /// This does NOT destroy the window or application documents, allowing
    /// application code to intercept, confirm with the user, or gracefully shut down.
    pub fn request_close(&mut self, id: WindowId) -> Result<(), PlatformError> {
        let win = self
            .windows
            .get_mut(&id)
            .ok_or(PlatformError::InvalidWindow(id))?;
        win.set_lifecycle(WindowLifecycle::CloseRequested);
        Ok(())
    }

    /// Explicitly close and destroy a native window.
    ///
    /// Cleans up modal hierarchies, releases held input buttons, and drops
    /// the underlying native window handle. Returns the pre-destruction state snapshot.
    pub fn close_window(
        &mut self,
        id: WindowId,
    ) -> Result<Option<WindowStateSnapshot>, PlatformError> {
        let snapshot = self.snapshot_window(id);

        let win = match self.windows.remove(&id) {
            Some(w) => w,
            None => return Err(PlatformError::InvalidWindow(id)),
        };

        // Clean up winit ID mapping
        self.winit_to_core.remove(&win.winit_id());
        self.input_trackers.remove(&id);
        self.window_titles.remove(&id);

        // Clean up modal relationships
        if let Some(parent_id) = win.modal_parent()
            && let Some(parent_win) = self.windows.get_mut(&parent_id)
        {
            parent_win.remove_modal_child(id);
        }
        for &child_id in win.modal_children() {
            if let Some(child_win) = self.windows.get_mut(&child_id) {
                child_win.set_modal_parent(None);
            }
        }

        // Dropping `win` releases `Arc<WinitWindow>`, closing native resources.
        Ok(snapshot)
    }

    /// Process an incoming winit `WindowEvent`, routing it safely to the framework.
    ///
    /// Late events targeting destroyed or unregistered windows return `None` safely.
    /// Modal-blocked windows drop pointer and keyboard input events.
    pub fn process_winit_event(
        &mut self,
        winit_id: WinitWindowId,
        event: &WindowEvent,
        timestamp_ms: u64,
    ) -> Option<(WindowId, PlatformWindowEvent)> {
        // Late event check: if window is not found in shell, drop safely!
        let window_id = self.lookup_id(winit_id)?;

        let win = self.windows.get_mut(&window_id)?;
        let scale_factor = win.dpi().scale_factor;

        // Update lifecycle & geometry on native updates
        match event {
            WindowEvent::CloseRequested => {
                win.set_lifecycle(WindowLifecycle::CloseRequested);
            }
            WindowEvent::Destroyed => {
                win.set_lifecycle(WindowLifecycle::Destroyed);
            }
            WindowEvent::Focused(true) => {
                win.set_lifecycle(WindowLifecycle::Active);
            }
            WindowEvent::Focused(false) => {
                win.set_lifecycle(WindowLifecycle::Inactive);
                // Clear any held pointer buttons on focus loss
                if let Some(tracker) = self.input_trackers.get_mut(&window_id) {
                    let _ = tracker.cancel_held_buttons();
                }
            }
            WindowEvent::Moved(_) => {
                if let Ok(pos) = win.raw_window().inner_position() {
                    win.set_screen_origin((pos.x, pos.y));
                }
            }
            WindowEvent::ScaleFactorChanged {
                scale_factor: factor,
                ..
            } => {
                if let Ok(scale) = ScaleFactor::new(*factor as f32) {
                    win.set_scale_factor(scale);
                }
                if let Ok(pos) = win.raw_window().inner_position() {
                    win.set_screen_origin((pos.x, pos.y));
                }
            }
            WindowEvent::Occluded(true) => {
                win.set_lifecycle(WindowLifecycle::Minimized);
            }
            WindowEvent::Occluded(false) => {
                win.set_lifecycle(WindowLifecycle::Active);
            }
            _ => {}
        }

        // Check modal blocking for pointer and keyboard inputs
        let is_blocked = win.is_blocked_by_modal();
        let tracker = self.input_trackers.get_mut(&window_id)?;

        let platform_event =
            translate_window_event(event, window_id, scale_factor, tracker, timestamp_ms)?;

        // Discard pointer and keyboard input if window is blocked by modal
        if is_blocked {
            match &platform_event {
                PlatformWindowEvent::Pointer(_) | PlatformWindowEvent::Keyboard(_) => {
                    return None;
                }
                _ => {}
            }
        }

        Some((window_id, platform_event))
    }
}
