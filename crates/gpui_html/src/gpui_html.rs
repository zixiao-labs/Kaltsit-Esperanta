//! HTML-like element construction for GPUI.
//!
//! This crate re-exports the [`html!`](html) macro and provides structural element
//! namespaces used by generated element trees. A structural namespace does not add
//! accessibility roles, hitboxes, focus behavior, or event handling.

use std::panic::Location;

use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Window,
};

#[doc(hidden)]
pub use gpui as __gpui;
pub use gpui_html_macros::html;

/// An element wrapper that establishes an outer [`ElementId`] namespace.
///
/// The wrapper is structural only. Its namespace key is not an interactive `.id()`:
/// it does not make the element interactive, create a hitbox, or add an accessibility
/// role. Layout, prepaint, and paint are delegated to the wrapped element while the
/// namespace is active.
pub struct NamespaceElement {
    namespace: ElementId,
    element: AnyElement,
}

/// Wraps an element in a structural namespace identified by `key`.
///
/// The key scopes descendant element IDs so structurally repeated subtrees can use
/// stable local IDs. It is not equivalent to an interactive `.id()` and does not add
/// hit-testing, focus behavior, event handling, or an accessibility role.
pub fn keyed(key: impl Into<ElementId>, element: impl IntoElement) -> NamespaceElement {
    NamespaceElement {
        namespace: key.into(),
        element: element.into_any_element(),
    }
}

/// Wraps an element in a structural namespace derived from the caller's code location.
///
/// This is equivalent to passing `ElementId::CodeLocation(*Location::caller())` to
/// [`keyed`]. The generated namespace is structural and is not an interactive `.id()`.
#[track_caller]
pub fn scoped(element: impl IntoElement) -> NamespaceElement {
    keyed(ElementId::CodeLocation(*Location::caller()), element)
}

impl IntoElement for NamespaceElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for NamespaceElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some(self.namespace.clone())
    }

    fn source_location(&self) -> Option<&'static Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        (self.element.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let _focus_handle = self.element.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.element.paint(window, cx);
    }
}
