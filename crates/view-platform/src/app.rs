//! Application runner integrating `winit`'s event loop and `PlatformShell`.

use view_core::WindowId;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::WindowId as WinitWindowId;

use crate::{
    com::ComGuard, error::PlatformError, events::PlatformWindowEvent, shell::PlatformShell,
};

/// Configuration options for launching the platform application.
#[derive(Clone, Debug, Default)]
pub struct AppConfig {
    /// Run in headless / hidden mode for automated testing or smoke runs.
    pub run_hidden: bool,
    /// Terminate after N iterations / idle cycles (useful for headless CI smoke).
    pub max_iterations: Option<u64>,
}

/// Handler trait for application callbacks dispatched by the platform shell.
pub trait PlatformHandler {
    /// Invoked once the platform event loop is ready to spawn the initial window(s).
    fn on_ready(
        &mut self,
        shell: &mut PlatformShell,
        event_loop: &ActiveEventLoop,
    ) -> Result<(), PlatformError>;

    /// Invoked on every normalized platform window event.
    fn on_window_event(
        &mut self,
        shell: &mut PlatformShell,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: PlatformWindowEvent,
    ) -> Result<(), PlatformError>;

    /// Invoked when the event loop is about to sleep / idle.
    fn on_idle(
        &mut self,
        _shell: &mut PlatformShell,
        _event_loop: &ActiveEventLoop,
    ) -> Result<(), PlatformError> {
        Ok(())
    }
}

/// Native application wrapper coordinating COM lifecycle, event pumping, and shell routing.
pub struct PlatformApp<H: PlatformHandler> {
    handler: H,
    shell: PlatformShell,
    config: AppConfig,
    iteration_count: u64,
    has_resumed: bool,
    #[allow(dead_code)]
    com_guard: Option<ComGuard>,
    start_time: std::time::Instant,
    failure: Option<PlatformError>,
}

impl<H: PlatformHandler> PlatformApp<H> {
    /// Create a new platform application instance with the given handler and config.
    pub fn new(handler: H, config: AppConfig) -> Result<Self, PlatformError> {
        #[cfg(windows)]
        let com_guard = Some(ComGuard::new()?);
        #[cfg(not(windows))]
        let com_guard = None;

        Ok(Self {
            handler,
            shell: PlatformShell::new(),
            config,
            iteration_count: 0,
            has_resumed: false,
            com_guard,
            start_time: std::time::Instant::now(),
            failure: None,
        })
    }

    /// Access the underlying platform shell.
    pub fn shell(&self) -> &PlatformShell {
        &self.shell
    }

    /// Mutably access the underlying platform shell.
    pub fn shell_mut(&mut self) -> &mut PlatformShell {
        &mut self.shell
    }

    /// Run the application event loop to completion.
    pub fn run(mut self) -> Result<(), PlatformError> {
        let event_loop = EventLoop::new()
            .map_err(|e| PlatformError::EventLoop(format!("Failed to create event loop: {e}")))?;
        event_loop.set_control_flow(ControlFlow::Wait);

        event_loop
            .run_app(&mut self)
            .map_err(|e| PlatformError::EventLoop(format!("Event loop run failed: {e}")))?;

        self.failure.map_or(Ok(()), Err)
    }
}

impl<H: PlatformHandler> ApplicationHandler for PlatformApp<H> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if !self.has_resumed {
            self.has_resumed = true;
            if let Err(err) = self.handler.on_ready(&mut self.shell, event_loop) {
                self.failure = Some(err);
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WinitWindowId,
        event: WindowEvent,
    ) {
        if self.failure.is_some() {
            return;
        }
        let timestamp_ms = self.start_time.elapsed().as_millis() as u64;

        if let Some((core_id, platform_event)) =
            self.shell
                .process_winit_event(window_id, &event, timestamp_ms)
            && let Err(err) =
                self.handler
                    .on_window_event(&mut self.shell, event_loop, core_id, platform_event)
        {
            self.failure = Some(err);
            event_loop.exit();
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.failure.is_some() {
            event_loop.exit();
            return;
        }
        self.iteration_count += 1;

        if let Err(err) = self.handler.on_idle(&mut self.shell, event_loop) {
            self.failure = Some(err);
            event_loop.exit();
            return;
        }

        // Check iteration limit in test/smoke runs
        if let Some(max) = self.config.max_iterations
            && self.iteration_count >= max
        {
            event_loop.exit();
            return;
        }

        // Exit if all windows have closed after initial setup
        if self.has_resumed && self.shell.is_empty() {
            event_loop.exit();
        }
    }
}
