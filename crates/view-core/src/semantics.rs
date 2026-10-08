//! Semantic tree representation, accessible roles/actions, and incremental snapshots.
//!
//! Exposes stable roles, names, values, and actions. Virtual and custom content
//! declares capabilities through explicit contracts.

use crate::{NodeId, Revision, geometry::LogicalRect};

/// Accessible role for a UI component.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    /// Window frame / surface.
    Window,
    /// Modal or modeless dialog.
    Dialog,
    /// Push button.
    Button,
    /// Toggle checkbox.
    Checkbox,
    /// Mutually exclusive radio button.
    Radio,
    /// Single or multi-line editable text input.
    TextInput,
    /// Non-interactive static label.
    StaticText,
    /// Scrollable viewport / area.
    ScrollArea,
    /// Generic structural container or group.
    Container,
    /// Continuous or stepped range slider.
    Slider,
    /// Custom component with named role.
    Custom(String),
}

/// Accessible actions a node can receive from assistive tech or automation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SemanticAction {
    /// Primary activation (click / press).
    Click,
    /// Request focus.
    Focus,
    /// Update value.
    SetValue,
    /// Scroll container.
    Scroll,
    /// Expand container / tree node.
    Expand,
    /// Collapse container / tree node.
    Collapse,
}

/// A semantic node published at commit time.
#[derive(Clone, Debug, PartialEq)]
pub struct SemanticNode {
    /// Node identity.
    pub id: NodeId,
    /// Component role.
    pub role: Role,
    /// Accessible name / label.
    pub name: Option<String>,
    /// Accessible value representation (e.g. text content or slider percentage).
    pub value: Option<String>,
    /// Supported semantic actions.
    pub actions: Vec<SemanticAction>,
    /// Bounding rectangle in window logical space.
    pub bounds: LogicalRect,
    /// Whether node is disabled.
    pub disabled: bool,
    /// Whether node has keyboard focus.
    pub focused: bool,
    /// Checkbox / radio checked state: Some(true), Some(false), or None if not checkable.
    pub checked: Option<bool>,
    /// Hidden from assistive technology.
    pub hidden: bool,
}

impl SemanticNode {
    /// Create a basic semantic node.
    pub fn new(id: NodeId, role: Role) -> Self {
        Self {
            id,
            role,
            name: None,
            value: None,
            actions: Vec::new(),
            bounds: LogicalRect::ZERO,
            disabled: false,
            focused: false,
            checked: None,
            hidden: false,
        }
    }
}

/// Incremental semantic updates published between revisions.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SemanticUpdate {
    /// Nodes that were added or modified since the previous snapshot.
    pub updated: Vec<SemanticNode>,
    /// Node identities that were removed since the previous snapshot.
    pub removed: Vec<NodeId>,
}

/// Coherent semantic snapshot at a specific commit revision.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SemanticSnapshot {
    /// Commit revision.
    pub revision: Revision,
    /// Semantic nodes in deterministic order.
    pub nodes: Vec<SemanticNode>,
}

impl SemanticSnapshot {
    /// Compute incremental difference against a previous semantic snapshot.
    pub fn diff(&self, previous: &Self) -> SemanticUpdate {
        let mut updated = Vec::new();
        let mut removed = Vec::new();

        // Check for modified or newly added nodes.
        for node in &self.nodes {
            if let Some(prev_node) = previous.nodes.iter().find(|n| n.id == node.id) {
                if prev_node != node {
                    updated.push(node.clone());
                }
            } else {
                updated.push(node.clone());
            }
        }

        // Check for removed nodes.
        for prev_node in &previous.nodes {
            if !self.nodes.iter().any(|n| n.id == prev_node.id) {
                removed.push(prev_node.id);
            }
        }

        SemanticUpdate { updated, removed }
    }
}

/// Contract for virtual or custom content to declare accessible semantics.
pub trait SemanticContract {
    /// Declare role, name, value, and supported actions.
    fn declare_semantics(&self, node_id: NodeId, bounds: LogicalRect) -> SemanticNode;
}
