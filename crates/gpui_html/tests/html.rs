#![deny(unused_parens)]

use gpui::{Element, ElementId, Empty, IntoElement, MouseButton, ParentElement, div, rgb};
use gpui_html::html;

fn assert_into_element(_: impl IntoElement) {}

#[test]
fn expands_intrinsic_elements_and_fluent_attributes() {
    let background = rgb(0x112233);

    let element = html! {
        <div
            flex
            flex_col
            bg={background}
            id={"root"}
            on_click={|_, _, _| {}}
            on_mouse_down(MouseButton::Left, |_, _, _| {})
        >
            "Heading"
            <text>{"Body"}</text>
            <svg path={"icons/check.svg"} id={"check-icon"} />
            <img source={"images/example.png"} />
        </div>
    };

    assert_into_element(element);
}

#[test]
fn expands_expression_and_control_flow_children() {
    let show_details = true;
    let optional = Some("Optional");
    let rows = ["First", "Second"];
    let expression_child = div().child("Expression");
    let conditional_text = html! {
        <text>{if show_details { "Shown" } else { "Hidden" }}</text>
    };

    let element = html! {
        <div>
            {expression_child}
            {if show_details {
                <text>{"Details"}</text>
                "Visible"
            } else {
                <div />
            }}
            {if let Some(label) = optional {
                <text>{label}</text>
            } else {
                "Missing"
            }}
            {for (index, label) in rows.into_iter().enumerate() {
                <div key={index}>
                    <text>{label}</text>
                </div>
            }}
        </div>
    };

    assert_into_element(element);
    assert_into_element(conditional_text);
}

#[test]
fn evaluates_structural_keys_before_moving_children() {
    let value = String::from("moved child");
    let element = html! {
        <div key={value.len()}>{value}</div>
    };

    assert_into_element(element);
}

#[test]
fn expands_dynamic_elements_with_implicit_and_explicit_scopes() {
    let implicit_scope = html! {
        <@{div()} flex>
            "Implicit scope"
        </@>
    };
    let explicit_scope = html! {
        <@{div()} key={"explicit"} flex />
    };

    assert_into_element(implicit_scope);
    assert_into_element(explicit_scope);
}

#[test]
fn dynamic_element_scopes_use_their_tag_locations() {
    let first = html! { <@{Empty} /> };
    let second = html! { <@{Empty} /> };

    assert!(matches!(first.id(), Some(ElementId::CodeLocation(_))));
    assert!(matches!(second.id(), Some(ElementId::CodeLocation(_))));
    assert_ne!(first.id(), second.id());
}
