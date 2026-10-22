//! WebView2 COM objects stay in this !Send UI-thread provider.
//! Every callback queues observations; it never pumps a nested loop or waits.
use crate::*;
use crate::{Error, Result};
use base64::Engine;
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::{Rc, Weak},
    sync::Arc,
};
use webview2_com::{Microsoft::Web::WebView2::Win32::*, *};
use windows::{
    Win32::{
        Foundation::{HWND, RECT},
        System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize},
        UI::Input::KeyboardAndMouse::{GetFocus, SetFocus},
        UI::WindowsAndMessaging::IsChild,
    },
    core::{HSTRING, Interface, PCWSTR, PWSTR},
};
use winit::{
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    window::Window,
};

fn native(e: windows::core::Error) -> Error {
    Error::Native(e.to_string())
}
struct Apartment;
impl Apartment {
    fn new() -> Result<Self> {
        // SAFETY: initializes this thread only, rejects incompatible apartments, balanced on Drop.
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED)
                .ok()
                .map_err(native)?;
        }
        Ok(Self)
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        // SAFETY: the Rc-containing provider cannot move threads; all COM references drop first.
        unsafe {
            CoUninitialize();
        }
    }
}
struct Environment {
    value: ICoreWebView2Environment,
    token: i64,
}
struct Panel {
    focused: Rc<Cell<bool>>,
    got_focus: i64,
    lost_focus: i64,
    html_navigation: Rc<RefCell<Option<String>>>,
    start: Start,
    controller: ICoreWebView2Controller,
    web: ICoreWebView2,
    nav: i64,
    completed: i64,
    message: i64,
    failed: i64,
    popup: i64,
    download: i64,
}
struct Inner {
    parents: BTreeMap<u64, Arc<Window>>,
    panels: BTreeMap<PanelId, Panel>,
    pending: BTreeMap<PanelId, Start>,
    environments: BTreeMap<SessionId, Environment>,
    events: Vec<Event>,
}
impl Inner {
    fn emit(&mut self, event: Event) {
        // Page traffic must never evict the exit receipt needed for safe cleanup.
        // Lifecycle entries coalesce per live panel/session; manager capacity bounds them.
        let same_lifecycle = |old: &Event| match (old, &event) {
            (Event::Ready(a, _), Event::Ready(b, _)) => a == b,
            (Event::Failed(a, _), Event::Failed(b, _)) => a == b,
            (Event::Exited(a, p), Event::Exited(b, q)) => a == b && p == q,
            _ => false,
        };
        if let Some(index) = self.events.iter().position(same_lifecycle) {
            self.events[index] = event;
            return;
        }
        let is_page_event = |e: &Event| {
            matches!(
                e,
                Event::Message(..) | Event::Navigation(..) | Event::NavigationStarting(..)
            )
        };
        if self.events.len() >= 256 {
            if let Some(index) = self.events.iter().position(is_page_event) {
                self.events.remove(index);
            } else if is_page_event(&event) {
                return;
            }
        }
        self.events.push(event);
    }
}
pub struct WindowsProvider {
    inner: Rc<RefCell<Inner>>,
    _apartment: Apartment,
}
pub fn runtime_version() -> Result<String> {
    let mut version = PWSTR::null();
    // SAFETY: WebView2 allocates a COM-task string into a valid output pointer.
    unsafe {
        GetAvailableCoreWebView2BrowserVersionString(PCWSTR::null(),&mut version).map_err(|e|Error::Unsupported(format!("WebView2 Evergreen runtime unavailable: {e}; install through the host's documented Microsoft Evergreen recovery route")))?;
    }
    Ok(CoTaskMemPWSTR::from(version).to_string())
}
impl WindowsProvider {
    pub fn new() -> Result<Self> {
        let apartment = Apartment::new()?;
        runtime_version()?;
        Ok(Self {
            inner: Rc::new(RefCell::new(Inner {
                parents: BTreeMap::new(),
                panels: BTreeMap::new(),
                pending: BTreeMap::new(),
                environments: BTreeMap::new(),
                events: vec![],
            })),
            _apartment: apartment,
        })
    }
    /// Retains the parent window until the provider drops. Call on the window's UI thread.
    pub fn register_parent(&mut self, key: u64, window: Arc<Window>) -> Result<()> {
        let handle = hwnd(&window)?;
        // SAFETY: winit retains a live HWND; these queries do not change window/thread state.
        unsafe {
            if windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(handle, None)
                != windows::Win32::System::Threading::GetCurrentThreadId()
            {
                return Err(Error::Invalid(
                    "parent belongs to a different UI thread".into(),
                ));
            }
        }
        if self.inner.borrow().parents.contains_key(&key) {
            return Err(Error::Invalid("parent key already registered".into()));
        }
        self.inner.borrow_mut().parents.insert(key, window);
        Ok(())
    }
    /// Host-only script API for fixtures and explicitly trusted application code.
    /// This is never exposed by the page bridge.
    pub fn execute_script(
        &mut self,
        id: PanelId,
        script: &str,
        done: impl FnOnce(Result<String>) + 'static,
    ) -> Result<()> {
        if script.len() > 65536 {
            return Err(Error::Budget);
        }
        let web = self
            .inner
            .borrow()
            .panels
            .get(&id)
            .ok_or(Error::Stale)?
            .web
            .clone();
        let bridge = self
            .inner
            .borrow()
            .panels
            .get(&id)
            .ok_or(Error::Stale)?
            .start
            .bridge
            .clone();
        let epoch = bridge.borrow().epoch();
        let callback = ExecuteScriptCompletedHandler::create(Box::new(move |error, value| {
            let valid = bridge.borrow().is_active() && bridge.borrow().epoch() == epoch;
            done(if valid {
                error.map(|_| value).map_err(native)
            } else {
                Err(Error::Stale)
            });
            Ok(())
        }));
        // SAFETY: cloned COM reference and callback live on this STA; script string lives through call.
        unsafe {
            web.ExecuteScript(&HSTRING::from(script), &callback)
                .map_err(native)
        }
    }
    pub fn navigate_html(&mut self, id: PanelId, html: &str) -> Result<()> {
        if html.len() > 1024 * 1024 {
            return Err(Error::Budget);
        }
        let inner = self.inner.borrow();
        let p = inner.panels.get(&id).ok_or(Error::Stale)?;
        if !p.start.policy.permits("about:blank") {
            return Err(Error::Denied);
        }
        p.start.bridge.borrow_mut().revoke();
        *p.html_navigation.borrow_mut() = Some(format!(
            "data:text/html;charset=utf-8;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(html)
        ));
        // SAFETY: owner-thread controller, bounded HTML; no external URL or profile is inferred.
        unsafe { p.web.NavigateToString(&HSTRING::from(html)).map_err(native) }
    }
    pub fn owned_counts(&self) -> (usize, usize, usize) {
        let i = self.inner.borrow();
        (i.panels.len(), i.environments.len(), i.pending.len())
    }
}
fn hwnd(window: &Window) -> Result<HWND> {
    match window
        .window_handle()
        .map_err(|e| Error::Native(e.to_string()))?
        .as_raw()
    {
        RawWindowHandle::Win32(handle) => Ok(HWND(handle.hwnd.get() as *mut _)),
        _ => Err(Error::Unsupported("Win32 parent required".into())),
    }
}
fn enqueue(weak: &Weak<RefCell<Inner>>, event: Event) {
    if let Some(i) = weak.upgrade() {
        i.borrow_mut().emit(event);
    }
}
impl Provider for WindowsProvider {
    fn start(&mut self, start: Start) -> Result<()> {
        let parent = hwnd(
            self.inner
                .borrow()
                .parents
                .get(&start.parent)
                .ok_or(Error::Stale)?,
        )?;
        start.placement.pixels()?;
        let path = HSTRING::from(start.profile.path().as_os_str());
        let id = start.panel;
        let session = start.session;
        let cancelled_bridge = start.bridge.clone();
        let shared = start.profile.shared;
        self.inner.borrow_mut().pending.insert(id, start);
        let weak = Rc::downgrade(&self.inner);
        let handler = CreateCoreWebView2EnvironmentCompletedHandler::create(Box::new(
            move |result, environment| {
                let result = (|| -> windows::core::Result<()> {
                    result?;
                    let environment = environment.ok_or_else(windows::core::Error::from_thread)?;
                    if shared && !cancelled_bridge.borrow().is_active() {
                        return Ok(());
                    }
                    let e5: ICoreWebView2Environment5 = environment.cast()?;
                    let exited = weak.clone();
                    let exit_handler =
                        BrowserProcessExitedEventHandler::create(Box::new(move |_, args| {
                            if let Some(args) = args {
                                let mut pid = 0;
                                // SAFETY: WebView2 invokes this on the environment STA with live args.
                                unsafe {
                                    args.BrowserProcessId(&mut pid)?;
                                }
                                enqueue(&exited, Event::Exited(session, pid));
                            }
                            Ok(())
                        }));
                    let mut token = 0;
                    // SAFETY: retained environment and callback are STA-owned; token is a valid out-param.
                    unsafe {
                        e5.add_BrowserProcessExited(&exit_handler, &mut token)?;
                    }
                    let Some(inner) = weak.upgrade() else {
                        return Ok(());
                    };
                    inner.borrow_mut().environments.insert(
                        session,
                        Environment {
                            value: environment.clone(),
                            token,
                        },
                    );
                    let controller_weak = weak.clone();
                    let controller_handler = CreateCoreWebView2ControllerCompletedHandler::create(
                        Box::new(move |result, controller| {
                            if let Err(e) = result {
                                enqueue(&controller_weak, Event::Failed(id, e.to_string()));
                                return Ok(());
                            }
                            if let Some(controller) = controller {
                                if let Err(e) = install(&controller_weak, id, controller) {
                                    enqueue(&controller_weak, Event::Failed(id, e.to_string()));
                                }
                            } else {
                                enqueue(
                                    &controller_weak,
                                    Event::Failed(id, "missing controller".into()),
                                );
                            }
                            Ok(())
                        }),
                    );
                    // SAFETY: parent Arc remains retained; environment and callback share this STA.
                    unsafe {
                        environment.CreateCoreWebView2Controller(parent, &controller_handler)?;
                    }
                    Ok(())
                })();
                if let Err(e) = result {
                    enqueue(&weak, Event::Failed(id, e.to_string()));
                }
                Ok(())
            },
        ));
        // SAFETY: null browser directory selects installed Evergreen; explicit owned profile path.
        let result = unsafe {
            CreateCoreWebView2EnvironmentWithOptions(PCWSTR::null(), &path, None, &handler)
                .map_err(native)
        };
        if result.is_err() {
            self.inner.borrow_mut().pending.remove(&id);
        }
        result
    }
    fn events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.inner.borrow_mut().events)
    }
    fn place(&mut self, id: PanelId, parent: u64, placement: &Placement) -> Result<()> {
        let [left, top, right, bottom] = placement.pixels()?;
        let mut i = self.inner.borrow_mut();
        let window = hwnd(i.parents.get(&parent).ok_or(Error::Stale)?)?;
        let p = i.panels.get_mut(&id).ok_or(Error::Stale)?;
        // SAFETY: parent retained by provider; physical client rectangle validated; owner-thread COM.
        unsafe {
            p.controller.SetParentWindow(window).map_err(native)?;
            p.controller
                .SetBounds(RECT {
                    left,
                    top,
                    right,
                    bottom,
                })
                .map_err(native)?;
            p.controller
                .NotifyParentWindowPositionChanged()
                .map_err(native)?;
        }
        p.start.parent = parent;
        Ok(())
    }
    fn visible(&mut self, id: PanelId, visible: bool) -> Result<()> {
        let i = self.inner.borrow();
        let p = i.panels.get(&id).ok_or(Error::Stale)?;
        // SAFETY: native control owns keyboard/IME/UIA while visible; focus leaves its child on hide.
        unsafe {
            if !visible && p.focused.get() {
                let parent = hwnd(i.parents.get(&p.start.parent).ok_or(Error::Stale)?)?;
                if IsChild(parent, GetFocus()).as_bool() {
                    let _ = SetFocus(Some(parent));
                }
            }
            p.controller.SetIsVisible(visible).map_err(native)
        }
    }
    fn focus(&mut self, id: PanelId) -> Result<()> {
        let i = self.inner.borrow();
        let p = i.panels.get(&id).ok_or(Error::Stale)?;
        // SAFETY: explicit host action on a live, visible controller, on its creating STA.
        unsafe {
            p.controller
                .MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC)
                .map_err(native)
        }
    }
    fn navigate(&mut self, id: PanelId, url: &str) -> Result<()> {
        let i = self.inner.borrow();
        let p = i.panels.get(&id).ok_or(Error::Stale)?;
        if !p.start.policy.permits(url) {
            return Err(Error::Denied);
        }
        // SAFETY: valid owner-thread COM reference; URL lifetime covers synchronous call.
        unsafe { p.web.Navigate(&HSTRING::from(url)).map_err(native) }
    }
    fn close(&mut self, id: PanelId) -> Result<()> {
        if self.inner.borrow().panels.contains_key(&id) {
            self.visible(id, false)?;
        }
        self.inner.borrow_mut().pending.remove(&id);
        let panel = self.inner.borrow_mut().panels.remove(&id);
        if let Some(p) = panel {
            close_panel(p)?;
        }
        Ok(())
    }
    fn release_session(&mut self, session: SessionId) {
        if let Some(e) = self.inner.borrow_mut().environments.remove(&session) {
            // SAFETY: on creating STA; token belongs to this environment.
            unsafe {
                if let Ok(e5) = e.value.cast::<ICoreWebView2Environment5>() {
                    let _ = e5.remove_BrowserProcessExited(e.token);
                }
            }
        }
    }
}
fn install(
    weak: &Weak<RefCell<Inner>>,
    id: PanelId,
    controller: ICoreWebView2Controller,
) -> Result<()> {
    let Some(inner) = weak.upgrade() else {
        // SAFETY: callback's live controller is on its creating STA, no owner remains.
        unsafe {
            controller.Close().map_err(native)?;
        }
        return Ok(());
    };
    let start = inner.borrow_mut().pending.remove(&id);
    let web;
    // SAFETY: successful creation callback hands out live controller on the STA.
    unsafe {
        web = controller.CoreWebView2().map_err(native)?;
    }
    let mut pid = 0;
    // SAFETY: live WebView2 and valid PID output.
    unsafe {
        web.BrowserProcessId(&mut pid).map_err(native)?;
    }
    inner.borrow_mut().emit(Event::Ready(id, pid));
    let Some(start) = start else {
        // SAFETY: a late callback is revoked; Close rather than resurrecting the panel.
        unsafe {
            controller.Close().map_err(native)?;
        }
        return Ok(());
    };
    let result = (|| -> windows::core::Result<Panel> {
        let mut p = Panel {
            focused: Rc::new(Cell::new(false)),
            got_focus: 0,
            lost_focus: 0,
            html_navigation: Rc::new(RefCell::new(None)),
            start: start.clone(),
            controller: controller.clone(),
            web: web.clone(),
            nav: 0,
            completed: 0,
            message: 0,
            failed: 0,
            popup: 0,
            download: 0,
        };
        let focused = p.focused.clone();
        let got_focus = FocusChangedEventHandler::create(Box::new(move |_, _| {
            focused.set(true);
            Ok(())
        }));
        let focused = p.focused.clone();
        let lost_focus = FocusChangedEventHandler::create(Box::new(move |_, _| {
            focused.set(false);
            Ok(())
        }));
        let nav_policy = start.policy.clone();
        let nav_bridge = start.bridge.clone();
        let html_navigation = p.html_navigation.clone();
        let nav_weak = weak.clone();
        let nav = NavigationStartingEventHandler::create(Box::new(move |_, args| {
            nav_bridge.borrow_mut().revoke();
            if let Some(args) = args {
                let mut uri = PWSTR::null();
                // SAFETY: live callback args; allocated URI transferred to RAII string.
                unsafe {
                    args.Uri(&mut uri)?;
                    let uri = CoTaskMemPWSTR::from(uri).to_string();
                    let expected = html_navigation.borrow_mut().take();
                    let allowed =
                        nav_policy.permits(&uri) || expected.as_deref() == Some(uri.as_str());
                    args.SetCancel(!allowed)?;
                    let diagnostic = if uri.starts_with("data:") {
                        "<host/local data document>".into()
                    } else {
                        uri.chars().take(4096).collect()
                    };
                    enqueue(
                        &nav_weak,
                        Event::NavigationStarting(id, diagnostic, allowed),
                    );
                }
            }
            Ok(())
        }));
        let complete_weak = weak.clone();
        let completed = NavigationCompletedEventHandler::create(Box::new(move |sender, args| {
            if let (Some(sender), Some(args)) = (sender, args) {
                let mut uri = PWSTR::null();
                let mut success = windows::core::BOOL(0);
                // SAFETY: callback-provided COM references and valid output pointers.
                unsafe {
                    sender.Source(&mut uri)?;
                    args.IsSuccess(&mut success)?;
                }
                enqueue(
                    &complete_weak,
                    Event::Navigation(id, CoTaskMemPWSTR::from(uri).to_string(), success.as_bool()),
                );
            }
            Ok(())
        }));
        let message_weak = weak.clone();
        let message_bridge = start.bridge.clone();
        let message = WebMessageReceivedEventHandler::create(Box::new(move |_, args| {
            if let Some(args) = args {
                let mut source = PWSTR::null();
                let mut json = PWSTR::null();
                // SAFETY: main-document event only (no Frame event subscriptions); strings use COM allocator.
                unsafe {
                    args.Source(&mut source)?;
                    args.WebMessageAsJson(&mut json)?;
                }
                let source = CoTaskMemPWSTR::from(source).to_string();
                let json = CoTaskMemPWSTR::from(json).to_string();
                if json.len() <= 65536
                    && source.len() <= 4096
                    && message_bridge.borrow().is_active()
                {
                    enqueue(&message_weak, Event::Message(id, source, json.into_bytes()));
                }
            }
            Ok(())
        }));
        let failed_weak = weak.clone();
        let failed_bridge = start.bridge.clone();
        let failed = ProcessFailedEventHandler::create(Box::new(move |_, _| {
            failed_bridge.borrow_mut().close();
            enqueue(
                &failed_weak,
                Event::Failed(
                    id,
                    "WebView2 process failure; hot instance invalidated".into(),
                ),
            );
            Ok(())
        }));
        let popup = NewWindowRequestedEventHandler::create(Box::new(move |_, args| {
            // SAFETY: reject implicit popup authority; app may navigate an explicitly created panel instead.
            unsafe {
                if let Some(args) = args {
                    args.SetHandled(true)?;
                }
            }
            Ok(())
        }));
        let downloads = start.policy.downloads;
        let download = DownloadStartingEventHandler::create(Box::new(move |_, args| {
            // SAFETY: callback args valid; download authority is a host-granted policy.
            unsafe {
                if let Some(args) = args {
                    args.SetCancel(!downloads)?;
                }
            }
            Ok(())
        }));
        let [left, top, right, bottom] = start
            .placement
            .pixels()
            .map_err(|_| windows::core::Error::from_thread())?;
        // SAFETY: all references and registrations remain on the controller's STA; Close removes them.
        unsafe {
            let settings = web.Settings()?;
            settings.SetAreDevToolsEnabled(start.policy.devtools)?;
            settings.SetAreHostObjectsAllowed(false)?;
            settings.SetAreDefaultContextMenusEnabled(false)?;
            controller.add_GotFocus(&got_focus, &mut p.got_focus)?;
            controller.add_LostFocus(&lost_focus, &mut p.lost_focus)?;
            web.add_NavigationStarting(&nav, &mut p.nav)?;
            web.add_NavigationCompleted(&completed, &mut p.completed)?;
            web.add_WebMessageReceived(&message, &mut p.message)?;
            web.add_ProcessFailed(&failed, &mut p.failed)?;
            web.add_NewWindowRequested(&popup, &mut p.popup)?;
            web.cast::<ICoreWebView2_4>()?
                .add_DownloadStarting(&download, &mut p.download)?;
            controller.SetBounds(RECT {
                left,
                top,
                right,
                bottom,
            })?;
            controller.SetIsVisible(true)?;
        }
        Ok(p)
    })();
    match result {
        Ok(panel) => {
            inner.borrow_mut().panels.insert(id, panel);
            Ok(())
        }
        Err(e) => {
            start.bridge.borrow_mut().close();
            // SAFETY: partial setup still owns the controller; Close breaks event/reference lifetime.
            unsafe {
                let _ = controller.Close();
            }
            Err(native(e))
        }
    }
}
fn close_panel(p: Panel) -> Result<()> {
    p.start.bridge.borrow_mut().close();
    // SAFETY: all registrations belong to this STA-owned controller. Close is always attempted,
    // even if removing an individual event fails. Drop then releases owned COM references.
    unsafe {
        let _ = p.controller.remove_GotFocus(p.got_focus);
        let _ = p.controller.remove_LostFocus(p.lost_focus);
        let _ = p.web.remove_NavigationStarting(p.nav);
        let _ = p.web.remove_NavigationCompleted(p.completed);
        let _ = p.web.remove_WebMessageReceived(p.message);
        let _ = p.web.remove_ProcessFailed(p.failed);
        let _ = p.web.remove_NewWindowRequested(p.popup);
        if let Ok(w) = p.web.cast::<ICoreWebView2_4>() {
            let _ = w.remove_DownloadStarting(p.download);
        }
        let _ = p.controller.SetIsVisible(false);
        p.controller.Close().map_err(native)
    }
}
impl Drop for WindowsProvider {
    fn drop(&mut self) {
        let panels = std::mem::take(&mut self.inner.borrow_mut().panels);
        for (_, p) in panels {
            let _ = close_panel(p);
        }
        let sessions: Vec<_> = self.inner.borrow().environments.keys().copied().collect();
        for id in sessions {
            self.release_session(id);
        }
        self.inner.borrow_mut().pending.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn page_flood_cannot_discard_cleanup_receipts() {
        let mut inner = Inner {
            parents: BTreeMap::new(),
            panels: BTreeMap::new(),
            pending: BTreeMap::new(),
            environments: BTreeMap::new(),
            events: vec![],
        };
        inner.emit(Event::Ready(PanelId(1), 42));
        inner.emit(Event::Exited(SessionId(1), 42));
        for _ in 0..1000 {
            inner.emit(Event::Message(PanelId(1), "about:blank".into(), vec![1]));
        }
        assert_eq!(inner.events.len(), 256);
        assert!(matches!(inner.events[0], Event::Ready(PanelId(1), 42)));
        assert!(matches!(inner.events[1], Event::Exited(SessionId(1), 42)));
    }
}
