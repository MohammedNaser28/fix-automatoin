use ratatui::style::Color;
pub struct Theme {
    pub background: Color,
    pub cyan: Color,
    pub green: Color,
    pub yellow: Color,
    pub red: Color,
    pub purple: Color,
    pub orange: Color,
    pub comment: Color, // dim text
    pub foreground: Color,
    pub selection: Color,
}

pub const THEME: Theme = Theme {
    background: Color::Rgb(13, 17, 23),
    cyan: Color::Rgb(121, 192, 255),
    green: Color::Rgb(86, 211, 100),
    yellow: Color::Rgb(227, 179, 65),
    red: Color::Rgb(248, 81, 73),
    purple: Color::Rgb(210, 168, 255),
    orange: Color::Rgb(255, 166, 87),
    comment: Color::Rgb(139, 148, 158),
    foreground: Color::Rgb(230, 237, 243),
    selection: Color::Rgb(31, 58, 95),
};
