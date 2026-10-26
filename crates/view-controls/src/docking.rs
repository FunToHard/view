//! Backend-independent docking, tear-off and snapshot contracts.
use crate::ControlError;
use std::collections::{HashMap, HashSet};
use view_core::{CoreError, NodeId, RoutedPointerAction, Runtime, Visibility, WindowId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplitAxis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DockNode {
    Stack {
        id: String,
        tabs: Vec<String>,
        active: String,
    },
    Split {
        axis: SplitAxis,
        ratio: f32,
        first: Box<DockNode>,
        second: Box<DockNode>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct DockWindow {
    pub id: String,
    pub root: DockNode,
    /// `[outer_x, outer_y, outer_width, outer_height]` in physical desktop pixels
    /// for native persistence. Hosts using other units must convert both saved
    /// bounds and work areas consistently before restoration.
    pub bounds: [f32; 4],
}

/// Cloneable persisted layout state. Hosts serialize this value using their chosen format.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DockSnapshot {
    pub windows: Vec<DockWindow>,
}

impl DockSnapshot {
    /// Encode a validated, versioned binary snapshot using length-prefixed UTF-8.
    pub fn encode(&self) -> Result<Vec<u8>, ControlError> {
        validate(self)?;
        let mut out = b"VDK1".to_vec();
        put_u32(&mut out, self.windows.len())?;
        for window in &self.windows {
            put_string(&mut out, &window.id)?;
            for value in window.bounds {
                out.extend_from_slice(&value.to_le_bytes());
            }
            encode_node(&mut out, &window.root)?;
        }
        Ok(out)
    }

    /// Decode and validate a persisted layout. Unknown versions, trailing bytes,
    /// excessive nesting, invalid UTF-8 and invalid dock invariants are rejected.
    pub fn decode(bytes: &[u8]) -> Result<Self, ControlError> {
        if bytes.len() > 1_048_576 {
            return Err(ControlError::Invalid("dock snapshot byte budget"));
        }
        let mut input = CodecReader { bytes, at: 0 };
        if input.take(4)? != b"VDK1" {
            return Err(ControlError::Invalid("dock snapshot version"));
        }
        let count = input.count()?;
        let mut windows = Vec::with_capacity(count);
        for _ in 0..count {
            let id = input.string()?;
            let mut bounds = [0.0; 4];
            for value in &mut bounds {
                *value = f32::from_le_bytes(input.take(4)?.try_into().expect("four bytes"));
            }
            windows.push(DockWindow {
                id,
                bounds,
                root: input.node(0)?,
            });
        }
        if input.at != bytes.len() {
            return Err(ControlError::Invalid("dock snapshot trailing bytes"));
        }
        let snapshot = Self { windows };
        validate(&snapshot)?;
        Ok(snapshot)
    }
}

fn put_u32(out: &mut Vec<u8>, value: usize) -> Result<(), ControlError> {
    let value =
        u32::try_from(value).map_err(|_| ControlError::Invalid("dock snapshot too large"))?;
    out.extend_from_slice(&value.to_le_bytes());
    Ok(())
}
fn put_string(out: &mut Vec<u8>, value: &str) -> Result<(), ControlError> {
    put_u32(out, value.len())?;
    out.extend_from_slice(value.as_bytes());
    Ok(())
}
fn encode_node(out: &mut Vec<u8>, node: &DockNode) -> Result<(), ControlError> {
    match node {
        DockNode::Stack { id, tabs, active } => {
            out.push(0);
            put_string(out, id)?;
            put_u32(out, tabs.len())?;
            for tab in tabs {
                put_string(out, tab)?;
            }
            put_string(out, active)?;
        }
        DockNode::Split {
            axis,
            ratio,
            first,
            second,
        } => {
            out.push(1);
            out.push(match axis {
                SplitAxis::Horizontal => 0,
                SplitAxis::Vertical => 1,
            });
            out.extend_from_slice(&ratio.to_le_bytes());
            encode_node(out, first)?;
            encode_node(out, second)?;
        }
    }
    Ok(())
}
struct CodecReader<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl CodecReader<'_> {
    fn take(&mut self, length: usize) -> Result<&[u8], ControlError> {
        let end = self
            .at
            .checked_add(length)
            .ok_or(ControlError::Invalid("dock snapshot length"))?;
        let value = self
            .bytes
            .get(self.at..end)
            .ok_or(ControlError::Invalid("truncated dock snapshot"))?;
        self.at = end;
        Ok(value)
    }
    fn u32(&mut self) -> Result<usize, ControlError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().expect("four bytes")) as usize)
    }
    fn count(&mut self) -> Result<usize, ControlError> {
        let count = self.u32()?;
        if count > 4096 || count > self.bytes.len() - self.at {
            return Err(ControlError::Invalid("dock snapshot item count"));
        }
        Ok(count)
    }
    fn string(&mut self) -> Result<String, ControlError> {
        let length = self.count()?;
        String::from_utf8(self.take(length)?.to_vec())
            .map_err(|_| ControlError::Invalid("dock snapshot UTF-8"))
    }
    fn node(&mut self, depth: usize) -> Result<DockNode, ControlError> {
        if depth > 128 {
            return Err(ControlError::Invalid("dock snapshot nesting"));
        }
        match self.take(1)?[0] {
            0 => {
                let id = self.string()?;
                let count = self.count()?;
                let mut tabs = Vec::with_capacity(count);
                for _ in 0..count {
                    tabs.push(self.string()?);
                }
                let active = self.string()?;
                Ok(DockNode::Stack { id, tabs, active })
            }
            1 => {
                let axis = match self.take(1)?[0] {
                    0 => SplitAxis::Horizontal,
                    1 => SplitAxis::Vertical,
                    _ => return Err(ControlError::Invalid("dock split axis")),
                };
                let ratio = f32::from_le_bytes(self.take(4)?.try_into().expect("four bytes"));
                let first = Box::new(self.node(depth + 1)?);
                let second = Box::new(self.node(depth + 1)?);
                Ok(DockNode::Split {
                    axis,
                    ratio,
                    first,
                    second,
                })
            }
            _ => Err(ControlError::Invalid("dock node tag")),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DockLayout {
    snapshot: DockSnapshot,
}

/// A mounted application panel. The host may keep a panel in a separate native
/// window after tear-off; each handle therefore carries its owning runtime window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DockPanelHandle {
    pub window: WindowId,
    pub root: NodeId,
}

/// An owner-thread command. Planning checks every model invariant before a host
/// creates windows or changes mounted panel ownership.
#[derive(Clone, Debug, PartialEq)]
pub enum DockCommand {
    MoveTab {
        tab: String,
        target_stack: String,
    },
    Activate {
        stack: String,
        tab: String,
    },
    TearOff {
        tab: String,
        window: String,
        bounds: [f32; 4],
    },
}

/// A panel whose runtime mount must be recreated in another native window.
/// Application document/editor state remains owned by the application.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DockPanelMigration {
    pub tab: String,
    pub source_window: String,
    pub target_window: String,
}

/// Validated model transaction. Stage fallible host window/mount creation first;
/// on failure dispose staged resources and keep the current layout. On success
/// cancel old capture, deliver cancellation, unmount old roots, then commit.
/// Native effects are host-owned and are not performed by this value.
#[derive(Clone, Debug, PartialEq)]
pub struct DockTransition {
    before: DockSnapshot,
    after: DockSnapshot,
    migrations: Vec<DockPanelMigration>,
}

impl DockTransition {
    pub fn snapshot(&self) -> &DockSnapshot {
        &self.after
    }
    pub fn migrations(&self) -> &[DockPanelMigration] {
        &self.migrations
    }
}

/// Taskbar/dock-excluded desktop rectangle in the same coordinate units as saved
/// window bounds. Native hosts should use physical desktop pixels consistently.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DockWorkArea {
    pub bounds: [f32; 4],
}

impl DockLayout {
    pub fn new(snapshot: DockSnapshot) -> Result<Self, ControlError> {
        validate(&snapshot)?;
        Ok(Self { snapshot })
    }
    pub fn snapshot(&self) -> DockSnapshot {
        self.snapshot.clone()
    }
    pub fn windows(&self) -> &[DockWindow] {
        &self.snapshot.windows
    }
    pub fn restore(snapshot: DockSnapshot) -> Result<Self, ControlError> {
        Self::new(snapshot)
    }

    /// Restore every window fully inside an available work area. Prefer greatest
    /// overlap with its saved bounds, otherwise the nearest area center; input
    /// order breaks ties. Clamp size (including minimum size) to that area, then
    /// clamp origin. Negative monitor coordinates are preserved. Missing monitors
    /// never leave an unreachable saved window. An empty monitor list is an error.
    pub fn restore_in_work_areas(
        mut snapshot: DockSnapshot,
        areas: &[DockWorkArea],
        minimum_size: [f32; 2],
    ) -> Result<Self, ControlError> {
        validate(&snapshot)?;
        if areas.is_empty()
            || areas.iter().any(|a| !valid_bounds(a.bounds))
            || minimum_size.iter().any(|n| !n.is_finite() || *n <= 0.0)
        {
            return Err(ControlError::Invalid("dock restoration work areas"));
        }
        for window in &mut snapshot.windows {
            let saved = window.bounds.map(f64::from);
            let mut selected = areas[0].bounds;
            let mut best_overlap = -1.0_f64;
            let mut best_distance = f64::INFINITY;
            for area in areas {
                let a = area.bounds.map(f64::from);
                let overlap = ((saved[0] + saved[2]).min(a[0] + a[2]) - saved[0].max(a[0]))
                    .max(0.0)
                    * ((saved[1] + saved[3]).min(a[1] + a[3]) - saved[1].max(a[1])).max(0.0);
                let distance = (saved[0] + saved[2] / 2.0 - a[0] - a[2] / 2.0).powi(2)
                    + (saved[1] + saved[3] / 2.0 - a[1] - a[3] / 2.0).powi(2);
                if overlap > best_overlap
                    || (overlap == 0.0 && best_overlap == 0.0 && distance < best_distance)
                {
                    best_overlap = overlap;
                    best_distance = distance;
                    selected = area.bounds;
                }
            }
            let width = window.bounds[2].max(minimum_size[0]).min(selected[2]);
            let height = window.bounds[3].max(minimum_size[1]).min(selected[3]);
            window.bounds = [
                window.bounds[0].clamp(selected[0], selected[0] + (selected[2] - width)),
                window.bounds[1].clamp(selected[1], selected[1] + (selected[3] - height)),
                width,
                height,
            ];
        }
        Self::new(snapshot)
    }

    pub fn plan(&self, command: DockCommand) -> Result<DockTransition, ControlError> {
        let mut next = self.clone();
        match command {
            DockCommand::MoveTab { tab, target_stack } => next.move_tab(&tab, &target_stack)?,
            DockCommand::Activate { stack, tab } => next.activate(&stack, &tab)?,
            DockCommand::TearOff {
                tab,
                window,
                bounds,
            } => next.tear_off(&tab, window, bounds)?,
        }
        let mut migrations = Vec::new();
        for window in &self.snapshot.windows {
            let mut tabs = HashSet::new();
            collect_tabs(&window.root, &mut tabs);
            let mut tabs: Vec<_> = tabs.into_iter().collect();
            tabs.sort();
            for tab in tabs {
                let (target_window, _) =
                    find_tab(&next.snapshot.windows, &tab).ok_or(ControlError::Stale)?;
                if target_window != window.id {
                    migrations.push(DockPanelMigration {
                        tab,
                        source_window: window.id.clone(),
                        target_window,
                    });
                }
            }
        }
        Ok(DockTransition {
            before: self.snapshot(),
            after: next.snapshot,
            migrations,
        })
    }

    /// Reject a plan built against another layout state before changing the model.
    /// Hosts must serialize plan/staging/commit on their UI thread.
    pub fn commit(&mut self, transition: DockTransition) -> Result<(), ControlError> {
        if self.snapshot != transition.before {
            return Err(ControlError::Stale);
        }
        self.snapshot = transition.after;
        Ok(())
    }

    /// Move a tab to an existing stack. Mutates only after all keys and destination
    /// conditions validate; empty source stacks/windows are retained for stable IDs.
    pub fn move_tab(&mut self, tab: &str, target_stack: &str) -> Result<(), ControlError> {
        let source = find_tab(&self.snapshot.windows, tab).ok_or(ControlError::Stale)?;
        find_stack(&self.snapshot.windows, target_stack).ok_or(ControlError::Stale)?;
        if source.1 == target_stack {
            return Ok(());
        }
        let mut next = self.snapshot.clone();
        remove_tab(&mut next.windows, tab);
        push_tab(&mut next.windows, target_stack, tab.to_owned());
        validate(&next)?;
        self.snapshot = next;
        Ok(())
    }

    pub fn activate(&mut self, stack_id: &str, tab: &str) -> Result<(), ControlError> {
        let mut next = self.snapshot.clone();
        let mut matched = false;
        for window in &mut next.windows {
            let mut contains = false;
            let found = mutate_stack(&mut window.root, stack_id, &mut |tabs, active| {
                if tabs.iter().any(|member| member == tab) {
                    *active = tab.to_owned();
                    contains = true;
                }
            });
            matched |= found && contains;
        }
        if !matched {
            return Err(ControlError::Stale);
        }
        validate(&next)?;
        self.snapshot = next;
        Ok(())
    }

    /// Tear a tab into a new host window with a caller-provided stable ID and bounds.
    pub fn tear_off(
        &mut self,
        tab: &str,
        window_id: impl Into<String>,
        bounds: [f32; 4],
    ) -> Result<(), ControlError> {
        let window_id = window_id.into();
        if find_tab(&self.snapshot.windows, tab).is_none() {
            return Err(ControlError::Stale);
        }
        if self.snapshot.windows.iter().any(|w| w.id == window_id) {
            return Err(ControlError::Invalid("duplicate dock window id"));
        }
        let mut next = self.snapshot.clone();
        remove_tab(&mut next.windows, tab);
        next.windows.push(DockWindow {
            id: window_id,
            root: DockNode::Stack {
                id: format!("stack:{tab}"),
                tabs: vec![tab.to_owned()],
                active: tab.to_owned(),
            },
            bounds,
        });
        validate(&next)?;
        self.snapshot = next;
        Ok(())
    }

    /// Reconcile panel visibility with active tabs. Focus is cleared and pointer
    /// capture cancellation actions are returned before inactive panel roots are
    /// suspended; callers deliver those actions to their control behavior sessions.
    pub fn reconcile_runtime<M, A>(
        &self,
        runtime: &mut Runtime<M, A>,
        panels: &HashMap<String, DockPanelHandle>,
    ) -> Result<Vec<RoutedPointerAction>, CoreError> {
        let mut active = HashSet::new();
        let mut expected_tabs = HashSet::new();
        for window in &self.snapshot.windows {
            collect_active(&window.root, &mut active);
            collect_tabs(&window.root, &mut expected_tabs);
        }
        // Validate the entire live handle set before changing any focus, capture
        // or visibility. A committed snapshot cannot establish current liveness.
        if expected_tabs.len() != panels.len()
            || expected_tabs.iter().any(|tab| !panels.contains_key(tab))
        {
            return Err(CoreError::InvalidPresentation);
        }
        let mut ordered: Vec<_> = panels.iter().collect();
        ordered.sort_by_key(|(tab, _)| *tab);
        for (index, (_, handle)) in ordered.iter().enumerate() {
            if runtime.node_window(handle.root)? != handle.window {
                return Err(CoreError::InvalidPresentation);
            }
            runtime.focused(handle.window)?;
            runtime.captured(handle.window)?;
            if ordered[..index].iter().any(|(_, other)| {
                other.root == handle.root
                    || runtime.is_descendant(other.root, handle.root)
                    || runtime.is_descendant(handle.root, other.root)
            }) {
                return Err(CoreError::InvalidPresentation);
            }
        }
        let mut cancelled = Vec::new();
        for (tab, handle) in ordered {
            if active.contains(tab) {
                runtime.set_visibility(handle.root, Visibility::Visible)?;
            } else {
                if let Some(focused) = runtime.focused(handle.window)?
                    && (focused == handle.root || runtime.is_descendant(handle.root, focused))
                {
                    runtime.set_focus(handle.window, None)?;
                }
                if let Some(captured) = runtime.captured(handle.window)?
                    && (captured == handle.root || runtime.is_descendant(handle.root, captured))
                {
                    cancelled.extend(runtime.cancel_pointer(handle.window)?);
                }
                runtime.set_visibility(handle.root, Visibility::Suspended)?;
            }
        }
        Ok(cancelled)
    }
}

fn collect_active(node: &DockNode, active: &mut HashSet<String>) {
    match node {
        DockNode::Stack {
            tabs,
            active: selected,
            ..
        } => {
            if tabs.iter().any(|tab| tab == selected) {
                active.insert(selected.clone());
            }
        }
        DockNode::Split { first, second, .. } => {
            collect_active(first, active);
            collect_active(second, active);
        }
    }
}
fn collect_tabs(node: &DockNode, tabs: &mut HashSet<String>) {
    match node {
        DockNode::Stack { tabs: members, .. } => tabs.extend(members.iter().cloned()),
        DockNode::Split { first, second, .. } => {
            collect_tabs(first, tabs);
            collect_tabs(second, tabs);
        }
    }
}

fn visit(node: &DockNode, f: &mut impl FnMut(&DockNode)) {
    f(node);
    if let DockNode::Split { first, second, .. } = node {
        visit(first, f);
        visit(second, f);
    }
}
fn find_tab(windows: &[DockWindow], tab: &str) -> Option<(String, String)> {
    for w in windows {
        let mut found = None;
        visit(&w.root, &mut |n| {
            if let DockNode::Stack { id, tabs, .. } = n
                && tabs.iter().any(|t| t == tab)
            {
                found = Some((w.id.clone(), id.clone()));
            }
        });
        if found.is_some() {
            return found;
        }
    }
    None
}
fn find_stack(windows: &[DockWindow], stack: &str) -> Option<()> {
    for w in windows {
        let mut found = false;
        visit(&w.root, &mut |n| {
            if let DockNode::Stack { id, .. } = n {
                found |= id == stack;
            }
        });
        if found {
            return Some(());
        }
    }
    None
}
fn mutate_stack(
    node: &mut DockNode,
    id: &str,
    action: &mut impl FnMut(&mut Vec<String>, &mut String),
) -> bool {
    match node {
        DockNode::Stack {
            id: own,
            tabs,
            active,
        } if own == id => {
            action(tabs, active);
            true
        }
        DockNode::Split { first, second, .. } => {
            mutate_stack(first, id, action) || mutate_stack(second, id, action)
        }
        _ => false,
    }
}
fn remove_tab(windows: &mut [DockWindow], tab: &str) {
    for w in windows {
        visit_mut(&mut w.root, &mut |tabs, active| {
            if let Some(i) = tabs.iter().position(|t| t == tab) {
                tabs.remove(i);
                if active == tab {
                    *active = tabs.first().cloned().unwrap_or_default();
                }
            }
        });
    }
}
fn visit_mut(node: &mut DockNode, f: &mut impl FnMut(&mut Vec<String>, &mut String)) {
    match node {
        DockNode::Stack { tabs, active, .. } => f(tabs, active),
        DockNode::Split { first, second, .. } => {
            visit_mut(first, f);
            visit_mut(second, f);
        }
    }
}
fn push_tab(windows: &mut [DockWindow], stack: &str, tab: String) {
    for w in windows {
        if mutate_stack(&mut w.root, stack, &mut |tabs, active| {
            tabs.push(tab.clone());
            *active = tab.clone();
        }) {
            return;
        }
    }
}
fn validate(snapshot: &DockSnapshot) -> Result<(), ControlError> {
    if snapshot.windows.len() > 4096 {
        return Err(ControlError::Invalid("dock window budget"));
    }
    // Bound recursion before calling the recursive traversal and codec helpers.
    let mut pending: Vec<_> = snapshot.windows.iter().map(|w| (&w.root, 0)).collect();
    let mut nodes = 0;
    let mut encoded_budget = 8;
    let mut tab_count = 0;
    for window in &snapshot.windows {
        if window.id.len() > 4096 {
            return Err(ControlError::Invalid("dock window id budget"));
        }
        encoded_budget += 20 + window.id.len();
    }
    while let Some((node, depth)) = pending.pop() {
        nodes += 1;
        if nodes > 4096 || depth > 128 {
            return Err(ControlError::Invalid("dock tree budget"));
        }
        match node {
            DockNode::Stack { id, tabs, active } => {
                if id.len() > 4096
                    || active.len() > 4096
                    || tabs.len() > 4096
                    || tabs.iter().any(|tab| tab.len() > 4096)
                {
                    return Err(ControlError::Invalid("dock string budget"));
                }
                tab_count += tabs.len();
                encoded_budget += 13
                    + id.len()
                    + active.len()
                    + tabs.iter().map(|tab| 4 + tab.len()).sum::<usize>();
            }
            DockNode::Split { first, second, .. } => {
                encoded_budget += 6;
                pending.push((first, depth + 1));
                pending.push((second, depth + 1));
            }
        }
        if encoded_budget > 1_048_576 || tab_count > 4096 {
            return Err(ControlError::Invalid("dock snapshot budget"));
        }
    }
    let mut windows = std::collections::HashSet::new();
    let mut stacks = std::collections::HashSet::new();
    let mut tabs = std::collections::HashSet::new();
    for w in &snapshot.windows {
        if w.id.is_empty() || !windows.insert(w.id.clone()) || !valid_bounds(w.bounds) {
            return Err(ControlError::Invalid("dock window"));
        }
        let mut error = false;
        visit(&w.root, &mut |n| match n {
            DockNode::Stack {
                id,
                tabs: members,
                active,
            } => {
                if id.is_empty()
                    || !stacks.insert(id.clone())
                    || (!members.is_empty() && !members.iter().any(|t| t == active))
                    || (members.is_empty() && !active.is_empty())
                {
                    error = true;
                }
                for t in members {
                    if t.is_empty() || !tabs.insert(t.clone()) {
                        error = true;
                    }
                }
            }
            DockNode::Split { ratio, .. } => {
                if !ratio.is_finite() || *ratio <= 0.0 || *ratio >= 1.0 {
                    error = true;
                }
            }
        });
        if error {
            return Err(ControlError::Invalid("dock tree"));
        }
    }
    Ok(())
}

fn valid_bounds(bounds: [f32; 4]) -> bool {
    bounds.iter().all(|value| value.is_finite())
        && bounds[2] > 0.0
        && bounds[3] > 0.0
        && (bounds[0] + bounds[2]).is_finite()
        && (bounds[1] + bounds[3]).is_finite()
}
