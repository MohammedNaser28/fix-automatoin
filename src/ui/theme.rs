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
    background: Color::Black,
    cyan: Color::Cyan,
    green: Color::Green,
    yellow: Color::Yellow,
    red: Color::Red,
    purple: Color::Magenta,
    orange: Color::LightRed,
    comment: Color::DarkGray,
    foreground: Color::White,
    selection: Color::Blue,
};
