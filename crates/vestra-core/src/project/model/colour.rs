#[must_use]
pub fn parse_colour(value: &str) -> Option<[u8; 4]> {
    let body = value.strip_prefix('#')?;
    if body.len() != 6 && body.len() != 8 {
        return None;
    }
    let red = u8::from_str_radix(&body[0..2], 16).ok()?;
    let green = u8::from_str_radix(&body[2..4], 16).ok()?;
    let blue = u8::from_str_radix(&body[4..6], 16).ok()?;
    let alpha = if body.len() == 8 {
        u8::from_str_radix(&body[6..8], 16).ok()?
    } else {
        255
    };
    Some([red, green, blue, alpha])
}
