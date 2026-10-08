//! Native decorated window wrapper and lifecycle management.

use std::sync::Arc;
use view_core::{LogicalSize, PhysicalSize, ScaleFactor, WindowId};
use winit::window::{Window as WinitWindow, WindowAttributes, WindowId as WinitWindowId};

#[cfg(windows)]
use windows::Win32::Foundation::HWND;
#[cfg(windows)]
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

use crate::{dpi::DpiState, error::PlatformError};

/// Lifecycle state of a native platform window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowLifecycle {
    /// Window created and opened.
    Opened,
    /// Window currently has input focus / active.
    Active,
    /// Window inactive / lost focus.
    Inactive,
    /// Window is minimized / occluded.
    Minimized,
    /// Close has been requested by user/OS.
    CloseRequested,
    /// Window destroyed and native resources released.
    Destroyed,
}

/// A framework-managed native window backed by winit and targeted Windows bindings.
pub struct PlatformWindow {
    id: WindowId,
    window: Arc<WinitWindow>,
    lifecycle: WindowLifecycle,
    dpi: DpiState,
    modal_parent: Option<WindowId>,
    modal_children: Vec<WindowId>,
}

impl PlatformWindow {
    /// Create default window attributes for a native decorated window.
    pub fn default_attributes(title: &str, size: LogicalSize) -> WindowAttributes {
        WinitWindow::default_attributes()
            .with_title(title)
            .with_decorations(true)
            .with_resizable(true)
            .with_inner_size(winit::dpi::LogicalSize::new(size.width, size.height))
    }

    /// Wrap an existing winit window with framework identity.
    pub fn new(id: WindowId, window: WinitWindow, text_scale: f32) -> Result<Self, PlatformError> {
        let scale = ScaleFactor::new(window.scale_factor() as f32).unwrap_or(ScaleFactor::ONE);
        let inner = window.inner_size();
        let client_pos = window
            .inner_position()
            .map_err(|e| PlatformError::WindowCreation(e.to_string()))?;

        let dpi = DpiState::new(scale, text_scale, (client_pos.x, client_pos.y));

        let _ = inner; // checked

        Ok(Self {
            id,
            window: Arc::new(window),
            lifecycle: WindowLifecycle::Opened,
            dpi,
            modal_parent: None,
            modal_children: Vec::new(),
        })
    }

    /// Framework window identity.
    #[inline]
    pub fn id(&self) -> WindowId {
        self.id
    }

    /// Winit window identity.
    #[inline]
    pub fn winit_id(&self) -> WinitWindowId {
        self.window.id()
    }

    /// Underlying winit window reference.
    #[inline]
    pub fn raw_window(&self) -> &WinitWindow {
        &self.window
    }

    /// Shared handle to underlying window.
    #[inline]
    pub fn shared_window(&self) -> Arc<WinitWindow> {
        self.window.clone()
    }

    /// Current lifecycle state.
    #[inline]
    pub fn lifecycle(&self) -> WindowLifecycle {
        self.lifecycle
    }

    /// Set lifecycle state.
    pub fn set_lifecycle(&mut self, state: WindowLifecycle) {
        self.lifecycle = state;
    }

    /// Current DPI state.
    #[inline]
    pub fn dpi(&self) -> DpiState {
        self.dpi
    }

    /// Update scale factor.
    pub fn set_scale_factor(&mut self, factor: ScaleFactor) {
        self.dpi.scale_factor = factor;
    }

    /// Update screen origin coordinates.
    pub fn set_screen_origin(&mut self, origin: (i32, i32)) {
        self.dpi.client_screen_origin = origin;
    }

    /// Current physical size of client area.
    pub fn physical_size(&self) -> PhysicalSize {
        let size = self.window.inner_size();
        PhysicalSize::from_pixels(size.width, size.height)
    }

    /// Current logical size of client area.
    pub fn logical_size(&self) -> LogicalSize {
        self.dpi.physical_to_logical_size(self.physical_size())
    }

    /// Request a redraw from the OS event loop.
    pub fn request_redraw(&self) {
        self.window.request_redraw();
    }

    /// Associated modal parent window, if this is a modal dialog.
    pub fn modal_parent(&self) -> Option<WindowId> {
        self.modal_parent
    }

    /// Set modal parent.
    pub fn set_modal_parent(&mut self, parent: Option<WindowId>) {
        self.modal_parent = parent;
    }

    /// Child modal windows currently open over this window.
    pub fn modal_children(&self) -> &[WindowId] {
        &self.modal_children
    }

    /// Register a child modal window.
    pub fn add_modal_child(&mut self, child: WindowId) {
        if !self.modal_children.contains(&child) {
            self.modal_children.push(child);
        }
    }

    /// Unregister a child modal window.
    pub fn remove_modal_child(&mut self, child: WindowId) {
        self.modal_children.retain(|&id| id != child);
    }

    /// True if any modal child is active over this window.
    pub fn is_blocked_by_modal(&self) -> bool {
        !self.modal_children.is_empty()
    }

    /// Extract the typed Win32 HWND handle on Windows.
    #[cfg(windows)]
    pub fn hwnd(&self) -> Option<HWND> {
        // SAFETY: window_handle() retrieves the RawWindowHandle representation from winit.
        // We verify that the handle variant is Win32 before casting the pointer to HWND.
        let handle = self.window.window_handle().ok()?;
        match handle.as_raw() {
            RawWindowHandle::Win32(win32_handle) => {
                let ptr = win32_handle.hwnd.get() as *mut std::ffi::c_void;
                Some(HWND(ptr))
            }
            _ => None,
        }
    }
}
