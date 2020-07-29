use crate::length::ScaleLength;
use crate::layout::SpacingKind;

use super::*;
use self::ContentKind::*;

function! {
    /// `line.break`, `n`: Ends the current line.
    #[derive(Debug, Default, Clone, PartialEq)]
    pub struct LineBreakFunc;

    parse(default)
    layout(self, ctx, f) { vec![BreakLine] }
}

function! {
    /// `par.break`: Ends the current paragraph.
    ///
    /// self has the same effect as two subsequent newlines.
    #[derive(Debug, Default, Clone, PartialEq)]
    pub struct ParBreakFunc;

    parse(default)
    layout(self, ctx, f) { vec![BreakParagraph] }
}

function! {
    /// `page.break`: Ends the current page.
    #[derive(Debug, Default, Clone, PartialEq)]
    pub struct PageBreakFunc;

    parse(default)
    layout(self, ctx, f) { vec![BreakPage] }
}

function! {
    /// `word.spacing`, `line.spacing`, `par.spacing`: The spacing between
    /// words, lines or paragraphs as a multiple of the font size.
    #[derive(Debug, Clone, PartialEq)]
    pub struct ContentSpacingFunc {
        body: Option<SyntaxModel>,
        content: ContentKind,
        spacing: Option<f64>,
    }

    type Meta = ContentKind;

    parse(header, body, state, f, meta) {
        ContentSpacingFunc {
            body: body!(opt: body, state, f),
            content: meta,
            spacing: header.args.pos.get::<f64>(&mut f.diagnostics)
                .map(|num| num as f64)
                .or_missing(&mut f.diagnostics, header.name.span, "spacing"),
        }
    }

    layout(self, ctx, f) {
        styled(&self.body, ctx, self.spacing, |t, s| match self.content {
            Word => t.word_spacing_scale = s,
            Line => t.line_spacing_scale = s,
            Paragraph => t.paragraph_spacing_scale = s,
        })
    }
}

/// The different kinds of content that can be spaced. Used as a metadata type
/// for the [`ContentSpacingFunc`].
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[allow(missing_docs)]
pub enum ContentKind {
    Word,
    Line,
    Paragraph,
}

function! {
    /// `spacing`, `h`, `v`: Adds spacing along an axis.
    #[derive(Debug, Clone, PartialEq)]
    pub struct SpacingFunc {
        spacing: Option<(AxisKey, ScaleLength)>,
    }

    type Meta = Option<SpecificAxis>;

    parse(header, body, state, f, meta) {
        body!(nope: body, f);
        SpacingFunc {
            spacing: if let Some(axis) = meta {
                header.args.pos.get::<ScaleLength>(&mut f.diagnostics)
                    .map(|s| (AxisKey::Specific(axis), s))
            } else {
                header.args.key.get_with_key::<AxisKey, ScaleLength>(&mut f.diagnostics)
            }.or_missing(&mut f.diagnostics, header.name.span, "spacing"),
        }
    }

    layout(self, ctx, f) {
        if let Some((axis, spacing)) = self.spacing {
            let axis = axis.to_generic(ctx.axes);
            let spacing = spacing.scaled(ctx.style.text.font_size());
            vec![AddSpacing(spacing, SpacingKind::Hard, axis)]
        } else {
            vec![]
        }
    }
}
