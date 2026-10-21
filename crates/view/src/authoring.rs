//! Canonical 0.1 Rust composition over the shared runtime. Descriptions are pure.
use crate::{CoreError, Description, Dirty, LogicalRect, NodeId, Role, Runtime, Structure};
use std::collections::HashSet;

/// Erasure occurs once per node. No mandatory macro, executor, or alternate identity.
pub struct Element<M, A> {
    description: Description<M, A>,
    children: Vec<Element<M, A>>,
}
pub trait IntoElement<M, A> {
    fn into_element(self) -> Element<M, A>;
}
impl<M, A> IntoElement<M, A> for Element<M, A> {
    fn into_element(self) -> Self {
        self
    }
}
impl<M, A> IntoElement<M, A> for Description<M, A> {
    fn into_element(self) -> Element<M, A> {
        Element::new(self)
    }
}
impl<M, A> Element<M, A> {
    /// Leaf ownership is retained by default. Nested authoring requires declarative ownership.
    pub fn new(description: Description<M, A>) -> Self {
        Self {
            description,
            children: vec![],
        }
    }
    pub fn layout(mut self, bounds: LogicalRect) -> Self {
        self.description = self.description.layout(bounds);
        self
    }
    pub fn children(mut self, children: impl Children<M, A>) -> Self {
        self.description = self.description.structure(Structure::Declarative);
        self.children = children.elements();
        self
    }
}
pub trait Children<M, A> {
    fn elements(self) -> Vec<Element<M, A>>;
}
impl<M, A> Children<M, A> for Vec<Element<M, A>> {
    fn elements(self) -> Vec<Element<M, A>> {
        self
    }
}
impl<M, A> Children<M, A> for () {
    fn elements(self) -> Vec<Element<M, A>> {
        vec![]
    }
}
macro_rules! tuple_children { ($($t:ident:$v:ident),+) => {
    impl<M,A,$($t: IntoElement<M,A>),+> Children<M,A> for ($($t,)+) {
        fn elements(self) -> Vec<Element<M,A>> { let ($($v,)+) = self; vec![$($v.into_element()),+] }
    }
}; }
tuple_children!(T0:a);
tuple_children!(T0:a,T1:b);
tuple_children!(T0:a,T1:b,T2:c);
tuple_children!(T0:a,T1:b,T2:c,T3:d);
tuple_children!(T0:a,T1:b,T2:c,T3:d,T4:e);
tuple_children!(T0:a,T1:b,T2:c,T3:d,T4:e,T5:f);

/// Dynamic children require caller-provided stable keys; duplicate keys fail before mutation.
pub fn keyed<M, A, T>(
    items: impl IntoIterator<Item = T>,
    build: impl FnMut(T) -> Element<M, A>,
) -> Vec<Element<M, A>> {
    items.into_iter().map(build).collect()
}

pub struct Text {
    key: String,
    text: String,
}
impl Text {
    pub fn new(key: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            text: text.into(),
        }
    }
}
struct TextState;
impl<M, A> IntoElement<M, A> for Text {
    fn into_element(self) -> Element<M, A> {
        Element::new(
            Description::new(self.key, TextState, |_, _, _| Dirty::NONE)
                .role(Role::StaticText)
                .name(self.text),
        )
    }
}
#[derive(Clone, Copy, Debug)]
pub struct ColumnProps {
    pub width: f32,
    pub row_height: f32,
    pub gap: f32,
}
impl Default for ColumnProps {
    fn default() -> Self {
        Self {
            width: 300.,
            row_height: 28.,
            gap: 4.,
        }
    }
}
pub struct Column<M, A> {
    key: String,
    props: ColumnProps,
    children: Vec<Element<M, A>>,
}
struct ColumnState;
impl<M, A> Column<M, A> {
    pub fn new(key: impl Into<String>, children: impl Children<M, A>) -> Self {
        Self {
            key: key.into(),
            props: ColumnProps::default(),
            children: children.elements(),
        }
    }
    pub fn with_props(mut self, props: ColumnProps) -> Self {
        self.props = props;
        self
    }
}
impl<M, A> IntoElement<M, A> for Column<M, A> {
    fn into_element(self) -> Element<M, A> {
        let p = self.props;
        let count = self.children.len();
        let children = self
            .children
            .into_iter()
            .enumerate()
            .map(|(i, c)| {
                c.layout(LogicalRect::from_xywh(
                    0.,
                    i as f32 * (p.row_height + p.gap),
                    p.width,
                    p.row_height,
                ))
            })
            .collect::<Vec<_>>();
        Element::new(
            Description::new(self.key, ColumnState, |_, _, _| Dirty::NONE).layout(
                LogicalRect::from_xywh(0., 0., p.width, count as f32 * (p.row_height + p.gap)),
            ),
        )
        .children(children)
    }
}

fn validate<M, A>(
    elements: &[Element<M, A>],
    depth: usize,
    count: &mut usize,
) -> Result<(), CoreError> {
    if depth > 64 {
        return Err(CoreError::CapacityExhausted);
    }
    let mut keys = HashSet::new();
    for e in elements {
        *count += 1;
        if *count > 10000 {
            return Err(CoreError::CapacityExhausted);
        }
        e.description.validate_geometry()?;
        if !keys.insert(e.description.key()) {
            return Err(CoreError::DuplicateKey);
        }
        validate(&e.children, depth + 1, count)?;
    }
    Ok(())
}
/// Reconcile nested owned descriptions. Retained leaves retain descendants and local state.
pub fn reconcile<M, A>(
    runtime: &mut Runtime<M, A>,
    parent: NodeId,
    children: impl Children<M, A>,
) -> Result<Vec<NodeId>, CoreError> {
    let elements = children.elements();
    validate(&elements, 0, &mut 0)?;
    reconcile_inner(runtime, parent, elements)
}
fn reconcile_inner<M, A>(
    runtime: &mut Runtime<M, A>,
    parent: NodeId,
    elements: Vec<Element<M, A>>,
) -> Result<Vec<NodeId>, CoreError> {
    let (descriptions, descendants): (Vec<_>, Vec<_>) = elements
        .into_iter()
        .map(|e| (e.description, e.children))
        .unzip();
    let ids = runtime.reconcile(parent, descriptions)?;
    for (&id, children) in ids.iter().zip(descendants) {
        // Empty declarative lists must also remove previously mounted children.
        if runtime.structure(id)? == Structure::Declarative {
            reconcile_inner(runtime, id, children)?;
        }
    }
    Ok(ids)
}
