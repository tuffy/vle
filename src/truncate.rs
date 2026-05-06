// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use ratatui::text::{Line, Span};
use std::borrow::Cow;
use std::collections::VecDeque;

/// Removes "chars" from the start of each line, if any
pub fn lines_start(mut lines: Vec<Line<'_>>, columns: usize) -> Vec<Line<'_>> {
    match columns {
        0 => lines,
        columns => {
            lines.iter_mut().for_each(|l| {
                let mut line = std::mem::take(&mut l.spans).into();
                truncate_spans(&mut line, columns);
                l.spans = line.into();
            });
            lines
        }
    }
}

pub fn line_start(mut line: Line<'_>, columns: usize) -> Line<'_> {
    match columns {
        0 => line,
        columns => {
            let mut spans = std::mem::take(&mut line.spans).into();
            truncate_spans(&mut spans, columns);
            line.spans = spans.into();
            line
        }
    }
}

fn truncate_spans(line: &mut VecDeque<Span<'_>>, mut columns: usize) {
    use unicode_width::UnicodeWidthChar;

    while columns > 0 {
        let Some(span) = line.pop_front() else {
            return;
        };
        let span_width: usize = span.content.chars().filter_map(|c| c.width()).sum();
        if span_width <= columns {
            columns -= span_width;
        } else {
            let suffix = truncate_cow(span.content, columns);
            line.push_front(Span {
                style: span.style,
                content: suffix,
            });
            return;
        }
    }
}

fn truncate_cow(s: Cow<'_, str>, mut columns: usize) -> Cow<'_, str> {
    use unicode_width::UnicodeWidthChar;

    let Some((split_point, _)) = s.char_indices().find(|(_, c)| {
        let width = c.width().unwrap_or(0);
        if width <= columns {
            columns -= width;
            false
        } else {
            true
        }
    }) else {
        return "".into();
    };

    match s {
        Cow::Borrowed(slice) => {
            let (_, end) = slice.split_at(split_point);
            match columns {
                0 => Cow::Borrowed(end),
                cols => Cow::Owned(
                    std::iter::repeat_n(' ', cols)
                        .chain(end.chars().skip(1))
                        .collect(),
                ),
            }
        }
        Cow::Owned(mut string) => {
            let suffix = string.split_off(split_point);
            match columns {
                0 => Cow::Owned(suffix),
                cols => Cow::Owned(
                    std::iter::repeat_n(' ', cols)
                        .chain(suffix.chars().skip(1))
                        .collect(),
                ),
            }
        }
    }
}
