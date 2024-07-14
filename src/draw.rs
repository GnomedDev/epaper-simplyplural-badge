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
    let style = FontTextStyleBuilder::new(font)
        .text_color(Color::Black)
        .font_size(50)
        .build();

    into_ok(Text::new(text, Point::new(20, 40), style).draw(display));
}
