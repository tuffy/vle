// Copyright 2026 Brian Langenberger
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use ratatui::{layout::Rect, widgets::StatefulWidget};
use std::ops::Range;

pub struct ScrollbarState {
    content_length: usize,
    position: usize,
    viewport_content_length: usize,
}

impl ScrollbarState {
    pub fn new(content_length: usize) -> Self {
        Self {
            content_length,
            position: 0,
            viewport_content_length: 0,
        }
    }

    pub fn viewport_content_length(self, viewport_content_length: usize) -> Self {
        Self {
            viewport_content_length,
            ..self
        }
    }

    pub fn position(self, position: usize) -> Self {
        Self { position, ..self }
    }

    fn thumb(&self) -> Option<Thumb<usize>> {
        // ensure start point of thumb doesn't push the thumb itself
        // outside of the scrollbar area
        (self.content_length > 0).then_some(Thumb {
            start: self.position.min(
                self.content_length
                    .saturating_sub(self.viewport_content_length),
            ),
            length: self.viewport_content_length.min(self.content_length),
        })
    }

    fn to_subpixels(&self, track_height: u16) -> impl Fn(usize) -> Subpixel {
        move |u| {
            // u as a percentage of content length
            let mut u = (u as f64) / (self.content_length as f64);
            // convert u to subpixels (8 subpixels per pixel)
            u *= (track_height as f64) * 8.0;
            // convert to a subpixels struct
            Subpixel::subpixels(u.round() as u32)
        }
    }
}

pub struct Scrollbar;

impl StatefulWidget for Scrollbar {
    type State = ScrollbarState;

    fn render(self, track: Rect, buf: &mut ratatui::buffer::Buffer, state: &mut Self::State) {
        use ratatui::style::Style;

        // ensure we're at least one pixel wide
        if track.width == 0 {
            return;
        }

        // determining max thumb position also checks whether
        // the track area is at least 1 pixel high
        let Some(max_thumb) = track.height.checked_sub(1).map(Subpixel::pixels) else {
            return;
        };

        match state.thumb() {
            Some(thumb) => {
                let thumb = thumb.map(state.to_subpixels(track.height));

                // constrain thumb to be at least 1 full pixel from the track's end
                // and to be at least 1 full pixel tall
                let thumb = Range::from(Thumb {
                    start: thumb.start.min(max_thumb),
                    length: thumb.length.max(Subpixel::pixels(1)),
                });

                // paint start of the thumb in subpixels
                buf[(track.x, track.y + thumb.start.pixel)]
                    .set_char(subpixels_char(thumb.start.subpixel));

                // paint whole blocks between start and end of thumb
                for i in (thumb.start.pixel + 1)..thumb.end.pixel {
                    buf[(track.x, track.y + i)].set_char('\u{2588}');
                }

                // paint end of thumb in inverted subpixels
                if thumb.end.subpixel > 0 {
                    buf[(track.x, track.y + thumb.end.pixel)]
                        .set_char(subpixels_char(thumb.end.subpixel));
                    buf[(track.x, track.y + thumb.end.pixel)]
                        .set_style(Style::default().reversed());
                }
            }
            None => {
                for i in 0..track.height {
                    buf[(track.x, track.y + i)].set_char('\u{2588}');
                }
            }
        }
    }
}

/// The scrollbar's thumb
struct Thumb<T> {
    start: T,
    length: T,
}

impl<T> Thumb<T> {
    fn map<U>(self, f: impl Fn(T) -> U) -> Thumb<U> {
        Thumb {
            start: f(self.start),
            length: f(self.length),
        }
    }
}

impl<T: Copy + std::ops::Add<Output = T>> From<Thumb<T>> for Range<T> {
    fn from(thumb: Thumb<T>) -> Self {
        Self {
            start: thumb.start,
            end: thumb.start + thumb.length,
        }
    }
}

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
struct Subpixel {
    pixel: u16,
    subpixel: u16,
}

impl Subpixel {
    fn pixels(pixel: u16) -> Self {
        Self { pixel, subpixel: 0 }
    }

    fn subpixels(subpixels: u32) -> Self {
        Self {
            pixel: (subpixels / 8) as u16,
            subpixel: (subpixels % 8) as u16,
        }
    }
}

impl std::ops::Add for Subpixel {
    type Output = Self;

    fn add(self, rhs: Subpixel) -> Self::Output {
        let total_subpixels = self.subpixel + rhs.subpixel;
        Self {
            pixel: self.pixel + rhs.pixel + total_subpixels / 8,
            subpixel: total_subpixels % 8,
        }
    }
}

fn subpixels_char(subpixels: u16) -> char {
    match subpixels {
        0 => '\u{2588}',
        1 => '\u{2587}',
        2 => '\u{2586}',
        3 => '\u{2585}',
        4 => '\u{2584}',
        5 => '\u{2583}',
        6 => '\u{2582}',
        7 => '\u{2581}',
        _ => '?',
    }
}
