use std::{cell::RefCell, panic::Location, rc::Rc};

use gpui::{
    App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId,
    Pixels, Style, TestAppContext, Window, point, px, size,
};
use gpui_html::keyed;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    RequestLayout,
    Prepaint,
    Paint,
}

struct CaptureElement {
    captured_ids: Rc<RefCell<Vec<(Phase, Option<String>)>>>,
}

impl CaptureElement {
    fn capture(&self, phase: Phase, id: Option<&GlobalElementId>) {
        self.captured_ids
            .borrow_mut()
            .push((phase, id.map(ToString::to_string)));
    }
}

impl IntoElement for CaptureElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for CaptureElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some("inner".into())
    }

    fn source_location(&self) -> Option<&'static Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        self.capture(Phase::RequestLayout, id);
        (window.request_layout(Style::default(), [], cx), ())
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
        self.capture(Phase::Prepaint, id);
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        _window: &mut Window,
        _cx: &mut App,
    ) {
        self.capture(Phase::Paint, id);
    }
}

#[gpui::test]
fn namespace_wraps_all_element_phases(cx: &mut TestAppContext) {
    let captured_ids = Rc::new(RefCell::new(Vec::new()));
    let visual_context = cx.add_empty_window();

    visual_context.draw(point(px(0.), px(0.)), size(px(100.), px(100.)), {
        let captured_ids = captured_ids.clone();
        move |_, _| keyed("outer", CaptureElement { captured_ids })
    });

    assert_eq!(
        captured_ids.borrow().as_slice(),
        [
            (Phase::RequestLayout, Some("outer.inner".to_owned())),
            (Phase::Prepaint, Some("outer.inner".to_owned())),
            (Phase::Paint, Some("outer.inner".to_owned())),
        ]
    );
}
