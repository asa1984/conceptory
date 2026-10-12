//! Extracts only the structure the checks need from a Markdown body.
//!
//! Both the checks and the viewer parse with comrak using [`options`], so that they never
//! interpret a document differently.

use comrak::{
    Arena, Options,
    nodes::{AstNode, NodeValue},
    parse_document,
};

#[derive(Debug, Default)]
pub struct Outline {
    pub headings: Vec<Heading>,
    pub links: Vec<Link>,
    /// Text written in bold
    pub strongs: Vec<String>,
    /// List items, including nested ones
    pub items: Vec<Block>,
    /// Paragraphs outside list items
    pub paragraphs: Vec<Block>,
}

#[derive(Debug)]
pub struct Heading {
    pub level: u8,
    pub text: String,
    pub line: usize,
}

#[derive(Debug)]
pub struct Link {
    pub dest: String,
    pub line: usize,
}

/// Text of an item or paragraph, with the preceding H2 and H3 as context.
#[derive(Debug)]
pub struct Block {
    pub text: String,
    pub line: usize,
    pub h2: Option<String>,
    pub h3: Option<String>,
}

/// comrak options matching GitHub's Markdown.
///
/// Raw HTML is passed through, so output used for display must be sanitized.
pub fn options() -> Options<'static> {
    let mut options = Options::default();
    let extension = &mut options.extension;
    extension.strikethrough = true;
    extension.table = true;
    extension.autolink = true;
    extension.tasklist = true;
    extension.footnotes = true;
    extension.alerts = true;
    extension.math_dollars = true;
    extension.math_code = true;
    extension.shortcodes = true;
    extension.front_matter_delimiter = Some("---".to_owned());
    extension.header_id_prefix = Some(String::new());
    options.render.r#unsafe = true;
    options.render.github_pre_lang = true;
    // Like GitHub, add the classes github-markdown-css uses to task lists
    options.render.tasklist_classes = true;
    options
}

/// State for attaching the preceding headings as context to the items and paragraphs after them.
#[derive(Default)]
struct Context {
    h2: Option<String>,
    h3: Option<String>,
    in_item: bool,
}

pub fn parse(source: &str) -> Outline {
    let arena = Arena::new();
    let root = parse_document(&arena, source, &options());
    let mut outline = Outline::default();
    let mut context = Context::default();
    visit(root, &mut context, &mut outline);
    outline
}

fn visit<'a>(node: &'a AstNode<'a>, context: &mut Context, outline: &mut Outline) {
    let line = node.data().sourcepos.start.line;
    let value = node.data().value.clone();
    match value {
        NodeValue::FrontMatter(_) => return,
        NodeValue::Heading(heading) => {
            let text = text_of(node);
            match heading.level {
                2 => {
                    context.h2 = Some(text.clone());
                    context.h3 = None;
                },
                3 => context.h3 = Some(text.clone()),
                _ => {},
            }
            outline.headings.push(Heading {
                level: heading.level,
                text,
                line,
            });
        },
        NodeValue::Link(target) => outline.links.push(Link {
            dest: target.url,
            line,
        }),
        NodeValue::Strong => outline.strongs.push(text_of(node)),
        NodeValue::Item(_) | NodeValue::TaskItem(_) => {
            outline.items.push(block(node, context, line));
            let outer = context.in_item;
            context.in_item = true;
            visit_children(node, context, outline);
            context.in_item = outer;
            return;
        },
        NodeValue::Paragraph if !context.in_item => {
            outline.paragraphs.push(block(node, context, line));
        },
        _ => {},
    }
    visit_children(node, context, outline);
}

fn visit_children<'a>(node: &'a AstNode<'a>, context: &mut Context, outline: &mut Outline) {
    for child in node.children() {
        visit(child, context, outline);
    }
}

fn block<'a>(node: &'a AstNode<'a>, context: &Context, line: usize) -> Block {
    Block {
        text: text_of(node),
        line,
        h2: context.h2.clone(),
        h3: context.h3.clone(),
    }
}

/// Concatenates the text under a node, turning line breaks into spaces
fn text_of<'a>(node: &'a AstNode<'a>) -> String {
    let mut text = String::new();
    for descendant in node.descendants() {
        match &descendant.data().value {
            NodeValue::Text(literal) => text.push_str(literal),
            NodeValue::Code(code) => text.push_str(&code.literal),
            NodeValue::SoftBreak | NodeValue::LineBreak => text.push(' '),
            _ => {},
        }
    }
    text.trim().to_owned()
}
