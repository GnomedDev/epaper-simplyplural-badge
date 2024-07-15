use core::convert::Infallible;

use embedded_font::FontTextStyleBuilder;
use embedded_graphics::{draw_target::DrawTarget, geometry::Point, text::Text, Drawable as _};
use epd_waveshare::color::Color;

use crate::EpdBuffer;

fn into_ok<T>(res: Result<T, Infallible>) -> T {
    match res {
        Ok(val) => val,
        Err(err) => match err {},
    }
}

pub fn clear_display(display: &mut EpdBuffer) {
    into_ok(display.clear(Color::White));
}

pub fn text_to_display(display: &mut EpdBuffer, font: rusttype::Font<'static>, text: &str) {
    let text = &text[..text.len().min(32)];
    let font_size = match text.len() {
        0..=9 => 50,
        10..=14 => 40,
        15..=19 => 30,
        20..=24 => 20,
        25..=32 => 15,
        _ => unreachable!(),
    };

    let style = FontTextStyleBuilder::new(font)
        .text_color(Color::Black)
        .font_size(font_size)
        .build();

    into_ok(Text::new(text, Point::new(20, 40), style).draw(display));
}
