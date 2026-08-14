use gpui::{Element, ElementId, Empty};
use gpui_html::{NamespaceElement, keyed, scoped};

fn assert_element(_: &impl Element) {}

#[test]
fn keyed_constructs_distinct_structural_namespaces() {
    let first = keyed("first", Empty);
    let second = keyed("second", Empty);

    assert_element(&first);
    assert_eq!(first.id(), Some(ElementId::from("first")));
    assert_eq!(second.id(), Some(ElementId::from("second")));
    assert_ne!(first.id(), second.id());
}

#[test]
fn keyed_has_a_public_concrete_return_type() {
    let _: NamespaceElement = keyed(7usize, Empty);
}

#[test]
fn scoped_uses_its_call_site_as_the_namespace() {
    let call_site_line = line!() + 1;
    let element = scoped(Empty);

    assert!(matches!(
        element.id(),
        Some(ElementId::CodeLocation(location))
            if location.file() == file!() && location.line() == call_site_line
    ));
}
