use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{quote, quote_spanned};
use syn::{
    Error, Expr, Ident, LitStr, Pat, Result, Token, braced, parenthesized,
    parse::{Parse, ParseStream, discouraged::Speculative},
    punctuated::Punctuated,
    spanned::Spanned,
};

#[proc_macro]
pub fn html(input: TokenStream) -> TokenStream {
    match expand_html(input.into()) {
        Ok(output) => output.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

fn expand_html(input: TokenStream2) -> Result<TokenStream2> {
    let facade = resolve_facade_path()?;
    expand_html_with_facade(input, &facade)
}

fn resolve_facade_path() -> Result<TokenStream2> {
    let found_crate = crate_name("gpui_html").map_err(|error| {
        Error::new(
            Span::call_site(),
            format!("failed to resolve the `gpui_html` facade crate: {error}"),
        )
    })?;

    match found_crate {
        FoundCrate::Itself => Ok(quote! { crate }),
        FoundCrate::Name(name) => {
            let identifier = syn::parse_str::<Ident>(&name).map_err(|error| {
                Error::new(
                    Span::call_site(),
                    format!("invalid resolved `gpui_html` crate name `{name}`: {error}"),
                )
            })?;
            Ok(quote! { ::#identifier })
        }
    }
}

fn expand_html_with_facade(input: TokenStream2, facade: &TokenStream2) -> Result<TokenStream2> {
    let input = syn::parse2::<HtmlInput>(input)?;
    let span = input.root.tag.span();
    let root = expand_element(&input.root, facade)?;

    Ok(quote_spanned! {span=>
        {
            #[allow(unused_imports)]
            use #facade::__gpui::prelude::*;
            #root
        }
    })
}

#[cfg(test)]
fn expand_html_for_test(input: TokenStream2) -> Result<TokenStream2> {
    expand_html_with_facade(input, &quote! { ::gpui_html })
}

struct HtmlInput {
    root: ElementNode,
}

impl Parse for HtmlInput {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        if input.is_empty() {
            return Err(input.error("html! requires exactly one root node; found none"));
        }

        if !input.peek(Token![<]) {
            return Err(input.error("html! root must be an element node"));
        }

        let root = input.parse()?;
        if !input.is_empty() {
            return Err(input.error("html! requires exactly one root node; found multiple"));
        }

        Ok(Self { root })
    }
}

struct ElementNode {
    tag: Tag,
    attributes: Vec<ElementAttribute>,
    children: Vec<Child>,
}

impl Parse for ElementNode {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let less_than: Token![<] = input.parse()?;
        let opening_span = less_than.span();

        if input.peek(Token![>]) {
            return Err(Error::new(
                opening_span,
                "fragment syntax is not supported; use a single element root",
            ));
        }

        if input.peek(Token![/]) {
            let lookahead = input.fork();
            let slash: Token![/] = lookahead.parse()?;
            if lookahead.peek(Token![>]) {
                return Err(Error::new(
                    slash.span(),
                    "fragment syntax is not supported; use a single element root",
                ));
            }

            return Err(Error::new(slash.span(), "unexpected closing tag"));
        }

        let tag = Tag::parse(input)?;
        let mut attributes = Vec::new();
        while !input.is_empty()
            && !input.peek(Token![>])
            && !(input.peek(Token![/]) && input.peek2(Token![>]))
        {
            attributes.push(input.parse()?);
        }

        if input.is_empty() {
            return Err(Error::new(tag.span(), "unterminated opening tag"));
        }

        let self_closing = if input.peek(Token![/]) {
            let slash: Token![/] = input.parse()?;
            if !input.peek(Token![>]) {
                return Err(Error::new(slash.span(), "expected `>` after `/`"));
            }
            input.parse::<Token![>]>()?;
            true
        } else {
            input.parse::<Token![>]>()?;
            false
        };

        validate_attributes(&tag, &attributes)?;

        let children = if self_closing {
            Vec::new()
        } else {
            let mut children = Vec::new();
            while !input.is_empty() && !is_closing_tag(input) {
                children.push(input.parse()?);
            }

            if input.is_empty() {
                return Err(Error::new(
                    tag.span(),
                    format!("missing closing tag for {}", tag.display_name()),
                ));
            }

            parse_closing_tag(input, &tag)?;
            children
        };

        validate_children(&tag, &children)?;

        Ok(Self {
            tag,
            attributes,
            children,
        })
    }
}

enum Tag {
    Intrinsic {
        kind: IntrinsicTag,
        identifier: Ident,
    },
    Dynamic {
        expression: Expr,
        span: Span,
    },
}

impl Tag {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        if input.peek(Token![@]) {
            let at: Token![@] = input.parse()?;
            if !input.peek(syn::token::Brace) {
                return Err(Error::new(
                    at.span(),
                    "dynamic tags must use `<@{expr}>` syntax",
                ));
            }

            let content;
            braced!(content in input);
            let expression = parse_single_expression(&content, "dynamic tag")?;
            return Ok(Self::Dynamic {
                expression,
                span: at.span(),
            });
        }

        let identifier = input.parse()?;
        let kind = IntrinsicTag::from_identifier(&identifier)?;
        Ok(Self::Intrinsic { kind, identifier })
    }

    fn span(&self) -> Span {
        match self {
            Self::Intrinsic { identifier, .. } => identifier.span(),
            Self::Dynamic { span, .. } => *span,
        }
    }

    fn display_name(&self) -> String {
        match self {
            Self::Intrinsic { identifier, .. } => format!("`<{identifier}>`"),
            Self::Dynamic { .. } => "dynamic tag".to_owned(),
        }
    }

    fn is_dynamic(&self) -> bool {
        matches!(self, Self::Dynamic { .. })
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum IntrinsicTag {
    Div,
    Svg,
    Img,
    Text,
}

impl IntrinsicTag {
    fn from_identifier(identifier: &Ident) -> Result<Self> {
        match identifier.to_string().as_str() {
            "div" => Ok(Self::Div),
            "svg" => Ok(Self::Svg),
            "img" => Ok(Self::Img),
            "text" => Ok(Self::Text),
            name => Err(Error::new(
                identifier.span(),
                format!("unknown intrinsic tag `<{name}>`; use `<@{{expr}}>` for dynamic elements"),
            )),
        }
    }
}

struct ElementAttribute {
    name: Ident,
    kind: AttributeKind,
}

impl ElementAttribute {
    fn span(&self) -> Span {
        self.name.span()
    }

    fn name(&self) -> String {
        self.name.to_string()
    }

    fn assignment_expression(&self, purpose: &str) -> Result<&Expr> {
        match &self.kind {
            AttributeKind::Assignment(expression) => Ok(expression),
            AttributeKind::Marker | AttributeKind::Call(_) => Err(Error::new(
                self.span(),
                format!("`{}` must use `{}={{expr}}` syntax", self.name(), purpose),
            )),
        }
    }

    fn expand_method_suffix(&self) -> TokenStream2 {
        let name = &self.name;
        match &self.kind {
            AttributeKind::Marker => quote_spanned! {self.span()=> .#name() },
            AttributeKind::Assignment(expression) => {
                quote_spanned! {self.span()=> .#name(#expression) }
            }
            AttributeKind::Call(arguments) => {
                quote_spanned! {self.span()=> .#name(#arguments) }
            }
        }
    }
}

impl Parse for ElementAttribute {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let name: Ident = input.parse()?;
        match name.to_string().as_str() {
            "class" => {
                return Err(Error::new(
                    name.span(),
                    "`class` attributes are not supported; use GPUI style methods",
                ));
            }
            "style" => {
                return Err(Error::new(
                    name.span(),
                    "`style` attributes are not supported; use GPUI style methods",
                ));
            }
            _ => {}
        }

        let kind = if input.peek(Token![=]) {
            input.parse::<Token![=]>()?;
            if !input.peek(syn::token::Brace) {
                return Err(Error::new(
                    name.span(),
                    format!("`{name}` values must use `{{expr}}` syntax"),
                ));
            }

            let content;
            braced!(content in input);
            AttributeKind::Assignment(parse_single_expression(&content, "attribute value")?)
        } else if input.peek(syn::token::Paren) {
            let content;
            parenthesized!(content in input);
            AttributeKind::Call(content.parse_terminated(Expr::parse, Token![,])?)
        } else {
            AttributeKind::Marker
        };

        Ok(Self { name, kind })
    }
}

enum AttributeKind {
    Marker,
    Assignment(Expr),
    Call(Punctuated<Expr, Token![,]>),
}

enum Child {
    Element(ElementNode),
    Literal(LitStr),
    Expression(Expr),
    If(IfChild),
    IfLet(IfLetChild),
    For(ForChild),
}

impl Child {
    fn span(&self) -> Span {
        match self {
            Self::Element(element) => element.tag.span(),
            Self::Literal(literal) => literal.span(),
            Self::Expression(expression) => expression.span(),
            Self::If(child) => child.span,
            Self::IfLet(child) => child.span,
            Self::For(child) => child.span,
        }
    }
}

impl Parse for Child {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        if input.peek(Token![<]) {
            return Ok(Self::Element(input.parse()?));
        }

        if input.peek(LitStr) {
            return Ok(Self::Literal(input.parse()?));
        }

        if input.peek(syn::token::Brace) {
            let content;
            braced!(content in input);
            let expression_input = content.fork();
            let expression_error =
                match parse_single_expression(&expression_input, "child expression") {
                    Ok(expression) => {
                        content.advance_to(&expression_input);
                        return Ok(Self::Expression(expression));
                    }
                    Err(error) => error,
                };

            let child = if content.peek(Token![if]) {
                parse_if_child(&content)?
            } else if content.peek(Token![for]) {
                Self::For(parse_for_child(&content)?)
            } else {
                return Err(expression_error);
            };

            if !content.is_empty() {
                return Err(content.error("unexpected tokens after child control flow"));
            }

            return Ok(child);
        }

        Err(input.error(
            "expected an element, a string literal, `{expr}`, `{if ...}`, or `{for ...}` child",
        ))
    }
}

struct IfChild {
    condition: Expr,
    then_children: Vec<Child>,
    else_children: Vec<Child>,
    span: Span,
}

struct IfLetChild {
    pattern: Pat,
    expression: Expr,
    then_children: Vec<Child>,
    else_children: Vec<Child>,
    span: Span,
}

struct ForChild {
    pattern: Pat,
    expression: Expr,
    children: Vec<Child>,
    span: Span,
}

fn parse_if_child(input: ParseStream<'_>) -> Result<Child> {
    let if_token: Token![if] = input.parse()?;
    let span = if_token.span();

    if input.peek(Token![let]) {
        input.parse::<Token![let]>()?;
        let pattern = input.call(Pat::parse_multi_with_leading_vert)?;
        input.parse::<Token![=]>()?;
        let expression = Expr::parse_without_eager_brace(input)?;
        let then_children = parse_braced_children(input)?;
        let else_children = parse_required_else(input)?;
        Ok(Child::IfLet(IfLetChild {
            pattern,
            expression,
            then_children,
            else_children,
            span,
        }))
    } else {
        let condition = Expr::parse_without_eager_brace(input)?;
        let then_children = parse_braced_children(input)?;
        let else_children = parse_required_else(input)?;
        Ok(Child::If(IfChild {
            condition,
            then_children,
            else_children,
            span,
        }))
    }
}

fn parse_for_child(input: ParseStream<'_>) -> Result<ForChild> {
    let for_token: Token![for] = input.parse()?;
    let pattern = input.call(Pat::parse_multi_with_leading_vert)?;
    input.parse::<Token![in]>()?;
    let expression = Expr::parse_without_eager_brace(input)?;
    let children = parse_braced_children(input)?;

    Ok(ForChild {
        pattern,
        expression,
        children,
        span: for_token.span(),
    })
}

fn parse_required_else(input: ParseStream<'_>) -> Result<Vec<Child>> {
    if !input.peek(Token![else]) {
        return Err(input.error("HTML child `if` expressions require an `else` branch"));
    }

    input.parse::<Token![else]>()?;
    if !input.peek(syn::token::Brace) {
        return Err(input.error("expected `{ ... }` after `else`"));
    }

    parse_braced_children(input)
}

fn parse_braced_children(input: ParseStream<'_>) -> Result<Vec<Child>> {
    let content;
    braced!(content in input);
    parse_children(&content)
}

fn parse_children(input: ParseStream<'_>) -> Result<Vec<Child>> {
    let mut children = Vec::new();
    while !input.is_empty() {
        children.push(input.parse()?);
    }
    Ok(children)
}

fn parse_single_expression(input: ParseStream<'_>, description: &str) -> Result<Expr> {
    if input.is_empty() {
        return Err(input.error(format!("{description} cannot be empty")));
    }

    let expression = input.parse()?;
    if !input.is_empty() {
        return Err(input.error(format!("unexpected tokens after {description} expression")));
    }
    Ok(expression)
}

fn is_closing_tag(input: ParseStream<'_>) -> bool {
    if !input.peek(Token![<]) {
        return false;
    }

    let lookahead = input.fork();
    if lookahead.parse::<Token![<]>().is_err() {
        return false;
    }
    lookahead.peek(Token![/])
}

fn parse_closing_tag(input: ParseStream<'_>, opening_tag: &Tag) -> Result<()> {
    input.parse::<Token![<]>()?;
    let slash: Token![/] = input.parse()?;

    if input.peek(Token![>]) {
        return Err(Error::new(
            slash.span(),
            "fragment syntax is not supported; closing tags must name their element",
        ));
    }

    match opening_tag {
        Tag::Intrinsic {
            identifier: opening_identifier,
            ..
        } => {
            if input.peek(Token![@]) {
                let at: Token![@] = input.parse()?;
                return Err(Error::new(
                    at.span(),
                    format!("closing tag does not match opening tag `<{opening_identifier}>`"),
                ));
            }

            let closing_identifier: Ident = input.parse()?;
            if closing_identifier != *opening_identifier {
                return Err(Error::new(
                    closing_identifier.span(),
                    format!(
                        "closing tag `</{closing_identifier}>` does not match opening tag `<{opening_identifier}>`"
                    ),
                ));
            }
        }
        Tag::Dynamic { .. } => {
            if !input.peek(Token![@]) {
                let closing_identifier: Ident = input.parse()?;
                return Err(Error::new(
                    closing_identifier.span(),
                    "dynamic tags must close with `</@>`",
                ));
            }

            let at: Token![@] = input.parse()?;
            if !input.peek(Token![>]) {
                return Err(Error::new(
                    at.span(),
                    "dynamic tags must close exactly with `</@>`",
                ));
            }
        }
    }

    input.parse::<Token![>]>()?;
    Ok(())
}

fn validate_attributes(tag: &Tag, attributes: &[ElementAttribute]) -> Result<()> {
    let is_img = matches!(
        tag,
        Tag::Intrinsic {
            kind: IntrinsicTag::Img,
            ..
        }
    );
    let mut id_attribute: Option<&ElementAttribute> = None;
    let mut key_attribute: Option<&ElementAttribute> = None;
    let mut source_attribute: Option<&ElementAttribute> = None;

    for attribute in attributes {
        match attribute.name().as_str() {
            "id" => {
                attribute.assignment_expression("id")?;
                if id_attribute.is_some() {
                    return Err(Error::new(attribute.span(), "duplicate `id` attribute"));
                }
                id_attribute = Some(attribute);
            }
            "key" => {
                attribute.assignment_expression("key")?;
                if key_attribute.is_some() {
                    return Err(Error::new(attribute.span(), "duplicate `key` attribute"));
                }
                key_attribute = Some(attribute);
            }
            "source" if is_img => {
                if source_attribute.is_some() {
                    return Err(Error::new(attribute.span(), "duplicate `source` attribute"));
                }
                source_attribute = Some(attribute);
            }
            _ => {}
        }
    }

    if is_img {
        let Some(source_attribute) = source_attribute else {
            return Err(Error::new(
                tag.span(),
                "`<img>` requires a `source={expr}` attribute",
            ));
        };
        source_attribute.assignment_expression("source")?;
    }

    Ok(())
}

fn validate_children(tag: &Tag, children: &[Child]) -> Result<()> {
    let Tag::Intrinsic { kind, .. } = tag else {
        return Ok(());
    };

    match kind {
        IntrinsicTag::Div => Ok(()),
        IntrinsicTag::Svg | IntrinsicTag::Img => {
            if let Some(child) = children.first() {
                Err(Error::new(
                    child.span(),
                    format!(
                        "{} is a leaf element and cannot have children",
                        tag.display_name()
                    ),
                ))
            } else {
                Ok(())
            }
        }
        IntrinsicTag::Text => {
            if children.len() != 1 {
                return Err(Error::new(
                    tag.span(),
                    format!(
                        "`<text>` requires exactly one string literal or `{{expr}}` child; found {}",
                        children.len()
                    ),
                ));
            }

            match children.first() {
                Some(Child::Literal(_) | Child::Expression(_)) => Ok(()),
                Some(child) => Err(Error::new(
                    child.span(),
                    "`<text>` is a leaf text element; its child must be a string literal or `{expr}`",
                )),
                None => Err(Error::new(
                    tag.span(),
                    "`<text>` requires exactly one child",
                )),
            }
        }
    }
}

fn expand_element(element: &ElementNode, facade: &TokenStream2) -> Result<TokenStream2> {
    let initialized_element = expand_initialized_element(element, facade)?;
    let span = element.tag.span();
    let unwrapped_element = if element.children.is_empty()
        || matches!(
            element.tag,
            Tag::Intrinsic {
                kind: IntrinsicTag::Text,
                ..
            }
        ) {
        initialized_element
    } else {
        let element_identifier = Ident::new("element", Span::mixed_site());
        let children = expand_children(&element.children, &element_identifier, facade)?;

        quote_spanned! {span=>
            {
                let mut #element_identifier = #initialized_element;
                #children
                #element_identifier
            }
        }
    };
    let key = structural_attribute_expression(&element.attributes, "key")?;

    Ok(expand_wrapper(
        &element.tag,
        key,
        unwrapped_element,
        span,
        facade,
    ))
}

fn expand_initialized_element(
    element: &ElementNode,
    facade: &TokenStream2,
) -> Result<TokenStream2> {
    let attributes = method_attributes(element);
    if let Tag::Dynamic { expression, span } = &element.tag {
        if attributes.is_empty() {
            return Ok(quote_spanned! {*span=> #expression });
        }

        let dynamic_element_identifier =
            Ident::new("__gpui_html_dynamic_element", Span::mixed_site());
        let mut output = quote_spanned! {*span=> #dynamic_element_identifier };
        for attribute in attributes {
            let suffix = attribute.expand_method_suffix();
            output = quote_spanned! {attribute.span()=> #output #suffix };
        }

        return Ok(quote_spanned! {*span=>
            {
                let #dynamic_element_identifier = #expression;
                #output
            }
        });
    }

    let mut output = expand_constructor(element, facade)?;
    for attribute in attributes {
        let suffix = attribute.expand_method_suffix();
        output = quote_spanned! {attribute.span()=> #output #suffix };
    }
    Ok(output)
}

fn expand_constructor(element: &ElementNode, facade: &TokenStream2) -> Result<TokenStream2> {
    match &element.tag {
        Tag::Dynamic { expression, span } => Ok(quote_spanned! {*span=> #expression }),
        Tag::Intrinsic { kind, identifier } => match kind {
            IntrinsicTag::Div => Ok(quote_spanned! {identifier.span()=> #facade::__gpui::div() }),
            IntrinsicTag::Svg => Ok(quote_spanned! {identifier.span()=> #facade::__gpui::svg() }),
            IntrinsicTag::Img => {
                let source = structural_attribute_expression(&element.attributes, "source")?
                    .ok_or_else(|| {
                        Error::new(
                            identifier.span(),
                            "`<img>` requires a `source={expr}` attribute",
                        )
                    })?;
                Ok(quote_spanned! {identifier.span()=> #facade::__gpui::img(#source) })
            }
            IntrinsicTag::Text => {
                let child = element.children.first().ok_or_else(|| {
                    Error::new(identifier.span(), "`<text>` requires exactly one child")
                })?;
                let text_span = child.span();
                let text = match child {
                    Child::Literal(literal) => quote_spanned! {literal.span()=> #literal },
                    Child::Expression(expression) => {
                        quote_spanned! {expression.span()=> #expression }
                    }
                    _ => {
                        return Err(Error::new(
                            child.span(),
                            "`<text>` is a leaf text element; its child must be a string literal or `{expr}`",
                        ));
                    }
                };

                if let Some(id) = structural_attribute_expression(&element.attributes, "id")? {
                    Ok(quote_spanned! {text_span=> #facade::__gpui::text!(id = #id, #text) })
                } else {
                    Ok(quote_spanned! {text_span=> #facade::__gpui::text!(#text) })
                }
            }
        },
    }
}

fn method_attributes(element: &ElementNode) -> Vec<&ElementAttribute> {
    let is_img = matches!(
        element.tag,
        Tag::Intrinsic {
            kind: IntrinsicTag::Img,
            ..
        }
    );
    let is_text = matches!(
        element.tag,
        Tag::Intrinsic {
            kind: IntrinsicTag::Text,
            ..
        }
    );

    element
        .attributes
        .iter()
        .filter(|attribute| {
            let name = attribute.name();
            name != "key" && !(is_img && name == "source") && !(is_text && name == "id")
        })
        .collect()
}

fn structural_attribute_expression<'a>(
    attributes: &'a [ElementAttribute],
    name: &str,
) -> Result<Option<&'a Expr>> {
    attributes
        .iter()
        .find(|attribute| attribute.name() == name)
        .map(|attribute| attribute.assignment_expression(name))
        .transpose()
}

fn expand_wrapper(
    tag: &Tag,
    key: Option<&Expr>,
    element: TokenStream2,
    span: Span,
    facade: &TokenStream2,
) -> TokenStream2 {
    if let Some(key) = key {
        let key_identifier = Ident::new("__gpui_html_key", Span::mixed_site());
        quote_spanned! {span=>
            {
                let #key_identifier = #key;
                #facade::keyed(#key_identifier, #element)
            }
        }
    } else if tag.is_dynamic() {
        quote_spanned! {span=> #facade::scoped(#element) }
    } else {
        element
    }
}

fn expand_children(
    children: &[Child],
    parent: &Ident,
    facade: &TokenStream2,
) -> Result<TokenStream2> {
    let mut output = TokenStream2::new();
    for child in children {
        output.extend(expand_child(child, parent, facade)?);
    }
    Ok(output)
}

fn expand_child(child: &Child, parent: &Ident, facade: &TokenStream2) -> Result<TokenStream2> {
    match child {
        Child::Element(element) => {
            let element = expand_element(element, facade)?;
            Ok(expand_concrete_child(element, child.span(), parent, facade))
        }
        Child::Literal(literal) => {
            let element = quote_spanned! {literal.span()=> #facade::__gpui::text!(#literal) };
            Ok(expand_concrete_child(
                element,
                literal.span(),
                parent,
                facade,
            ))
        }
        Child::Expression(expression) => Ok(expand_concrete_child(
            quote_spanned! {expression.span()=> #expression },
            expression.span(),
            parent,
            facade,
        )),
        Child::If(child) => {
            let condition = &child.condition;
            let then_children = expand_children(&child.then_children, parent, facade)?;
            let else_children = expand_children(&child.else_children, parent, facade)?;
            Ok(quote_spanned! {child.span=>
                if #condition {
                    #then_children
                } else {
                    #else_children
                }
            })
        }
        Child::IfLet(child) => {
            let pattern = &child.pattern;
            let expression = &child.expression;
            let then_children = expand_children(&child.then_children, parent, facade)?;
            let else_children = expand_children(&child.else_children, parent, facade)?;
            Ok(quote_spanned! {child.span=>
                if let #pattern = #expression {
                    #then_children
                } else {
                    #else_children
                }
            })
        }
        Child::For(child) => {
            let pattern = &child.pattern;
            let expression = &child.expression;
            let children = expand_children(&child.children, parent, facade)?;
            Ok(quote_spanned! {child.span=>
                for #pattern in #expression {
                    #children
                }
            })
        }
    }
}

fn expand_concrete_child(
    element: TokenStream2,
    span: Span,
    parent: &Ident,
    facade: &TokenStream2,
) -> TokenStream2 {
    quote_spanned! {span=>
        #facade::__gpui::ParentElement::extend(
            &mut #parent,
            ::core::iter::once(#facade::__gpui::IntoElement::into_any_element(#element)),
        );
    }
}

#[cfg(test)]
mod tests {
    use proc_macro2::TokenStream as TokenStream2;
    use quote::quote;

    use super::expand_html_for_test;

    fn expanded(input: TokenStream2) -> String {
        match expand_html_for_test(input) {
            Ok(output) => output.to_string(),
            Err(error) => error.to_compile_error().to_string(),
        }
    }

    fn error(input: TokenStream2) -> String {
        match expand_html_for_test(input) {
            Ok(output) => format!("unexpected success: {output}"),
            Err(error) => error.to_string(),
        }
    }

    #[test]
    fn preserves_intrinsic_fluent_attribute_source_order() {
        let output = expanded(quote! {
            <div bg={background} flex id={identifier} on_mouse_down(button, handler) />
        });

        assert!(output.contains("use :: gpui_html :: __gpui :: prelude :: *"));
        assert!(output.contains(
            ":: gpui_html :: __gpui :: div () . bg (background) . flex () . id (identifier) . on_mouse_down (button , handler)"
        ));
        assert!(!output.contains(":: gpui ::"));
    }

    #[test]
    fn dynamic_attributes_use_a_mixed_site_element_temporary() {
        let output = expanded(quote! {
            <@{if condition { first } else { second }}
                bg={background}
                id={identifier}
                flex
            />
        });

        assert!(output.contains(":: gpui_html :: scoped"));
        assert!(
            output.contains(
                "let __gpui_html_dynamic_element = if condition { first } else { second }"
            )
        );
        assert!(
            output.contains(
                "__gpui_html_dynamic_element . bg (background) . id (identifier) . flex ()"
            )
        );
        assert!(!output.contains("= (if condition"));
    }

    #[test]
    fn dynamic_tag_without_attributes_is_a_direct_expression() {
        let output = expanded(quote! {
            <@{Empty} />
        });

        assert!(output.contains(":: gpui_html :: scoped (Empty)"));
        assert!(!output.contains("__gpui_html_dynamic_element"));
        assert!(!output.contains("scoped ((Empty))"));
    }

    #[test]
    fn keyed_elements_use_a_mixed_site_key_temporary() {
        let output = expanded(quote! {
            <@{make_element()} key={item_key} />
        });

        assert!(output.contains("let __gpui_html_key = item_key"));
        assert!(output.contains(":: gpui_html :: keyed (__gpui_html_key , make_element ())"));
        assert!(!output.contains(":: gpui_html :: scoped"));
    }

    #[test]
    fn keyed_parent_evaluates_key_before_the_unwrapped_element_block() {
        let output = expanded(quote! {
            <div key={make_key()}>{make_child()}</div>
        });
        let key_position = output.find("let __gpui_html_key = make_key ()");
        let wrapper_position = output.find(":: gpui_html :: keyed (__gpui_html_key");
        let constructor_position = output.find(":: gpui_html :: __gpui :: div ()");
        let child_position = output.find("into_any_element (make_child ())");

        assert!(key_position.is_some());
        assert!(wrapper_position.is_some());
        assert!(constructor_position.is_some());
        assert!(child_position.is_some());
        assert!(key_position < wrapper_position);
        assert!(wrapper_position < constructor_position);
        assert!(constructor_position < child_position);
        assert!(output.contains("keyed (__gpui_html_key , { let mut element ="));
    }

    #[test]
    fn expands_img_and_text_constructors_through_the_facade() {
        let image = expanded(quote! { <img source={image_source} /> });
        let text = expanded(quote! { <text>{label}</text> });
        let identified_text = expanded(quote! { <text id={identifier}>{label}</text> });

        assert!(image.contains(":: gpui_html :: __gpui :: img (image_source)"));
        assert!(text.contains(":: gpui_html :: __gpui :: text ! (label)"));
        assert!(
            identified_text.contains(":: gpui_html :: __gpui :: text ! (id = identifier , label)")
        );
        assert!(!identified_text.contains(") . id (identifier)"));
    }

    #[test]
    fn parses_complete_rust_if_as_a_text_expression() {
        let output = expanded(quote! {
            <text>{if condition { "yes" } else { "no" }}</text>
        });

        assert!(output.contains(
            ":: gpui_html :: __gpui :: text ! (if condition { \"yes\" } else { \"no\" })"
        ));
        assert!(!output.contains("ParentElement :: extend"));
    }

    #[test]
    fn falls_back_to_dsl_control_flow_for_multi_child_bodies() {
        let output = expanded(quote! {
            <div>
                "before"
                {if condition {
                    <div />
                    {first}
                } else {
                    "fallback"
                }}
                {if let Some(value) = optional {
                    {value}
                    <div />
                } else {
                    {empty}
                }}
                {for item in items {
                    <text>{item}</text>
                    {separator}
                }}
            </div>
        });

        assert!(output.contains("let mut element = :: gpui_html :: __gpui :: div ()"));
        assert!(output.contains(":: gpui_html :: __gpui :: ParentElement :: extend"));
        assert!(output.contains(":: gpui_html :: __gpui :: IntoElement :: into_any_element"));
        assert!(output.contains("if condition"));
        assert!(output.contains("if let Some (value) = optional"));
        assert!(output.contains("for item in items"));
        assert!(output.contains(":: gpui_html :: __gpui :: text ! (\"before\")"));
    }

    #[test]
    fn does_not_parenthesize_expression_children() {
        let output = expanded(quote! {
            <div>{child}</div>
        });

        assert!(output.contains("into_any_element (child)"));
        assert!(!output.contains("into_any_element ((child))"));
    }

    #[test]
    fn accepts_dynamic_elements_with_children() {
        let output = expanded(quote! {
            <@{make_parent()}>{child}</@>
        });

        assert!(output.contains("let mut element = make_parent ()"));
        assert!(output.contains(":: gpui_html :: scoped ({ let mut element ="));
    }

    #[test]
    fn duplicate_source_is_a_regular_method_outside_img() {
        let output = expanded(quote! {
            <div source={first} source={second} />
        });

        assert!(
            output.contains(":: gpui_html :: __gpui :: div () . source (first) . source (second)")
        );
    }

    #[test]
    fn reports_root_tag_and_leaf_errors() {
        let cases = [
            (quote! {}, "exactly one root node; found none"),
            (
                quote! { <div /><div /> },
                "exactly one root node; found multiple",
            ),
            (quote! { <div></svg> }, "does not match opening tag"),
            (
                quote! { <@{make_parent()}></@{make_parent()}> },
                "dynamic tags must close exactly with `</@>`",
            ),
            (quote! { <button /> }, "unknown intrinsic tag"),
            (
                quote! { <svg>{child}</svg> },
                "leaf element and cannot have children",
            ),
            (quote! { <></> }, "fragment syntax is not supported"),
        ];

        for (input, expected) in cases {
            assert!(error(input).contains(expected), "expected `{expected}`");
        }
    }

    #[test]
    fn reports_structural_attribute_errors() {
        let cases = [
            (quote! { <img /> }, "requires a `source={expr}` attribute"),
            (
                quote! { <img source={first} source={second} /> },
                "duplicate `source` attribute",
            ),
            (
                quote! { <div id={first} id={second} /> },
                "duplicate `id` attribute",
            ),
            (
                quote! { <div key={first} key={second} /> },
                "duplicate `key` attribute",
            ),
            (
                quote! { <div class={classes} /> },
                "`class` attributes are not supported",
            ),
            (
                quote! { <div style={styles} /> },
                "`style` attributes are not supported",
            ),
        ];

        for (input, expected) in cases {
            assert!(error(input).contains(expected), "expected `{expected}`");
        }
    }

    #[test]
    fn reports_text_child_errors() {
        assert!(error(quote! { <text /> }).contains("requires exactly one"));
        assert!(error(quote! { <text>"one" "two"</text> }).contains("requires exactly one"));
        assert!(error(quote! { <text><div /></text> }).contains("leaf text element"));
    }
}
