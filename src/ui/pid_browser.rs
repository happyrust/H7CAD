use crate::app::Message;
use crate::io::pid_import::PidNodeKey;
use iced::widget::{button, column, container, row, scrollable, text, text_input};
use iced::{Background, Border, Color, Element, Length, Theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PidBrowserSection {
    Overview,
    Objects,
    Relationships,
    Sheets,
    Streams,
    CrossRef,
}

impl PidBrowserSection {
    pub fn label(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Objects => "Objects",
            Self::Relationships => "Relationships",
            Self::Sheets => "Sheets",
            Self::Streams => "Streams",
            Self::CrossRef => "CrossRef",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PidBrowserListItem {
    pub key: PidNodeKey,
    pub title: String,
    pub subtitle: Option<String>,
    pub badge: Option<String>,
}

pub fn view_panel<'a>(
    active_section: PidBrowserSection,
    search_text_value: &'a str,
    selected_key: Option<PidNodeKey>,
    items: Vec<PidBrowserListItem>,
    empty_hint: &'a str,
) -> Element<'a, Message> {
    let header = container(text("P&ID Browser").size(12).color(Color::WHITE))
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(HEADER_BG)),
            ..Default::default()
        })
        .padding([6, 10]);

    let sections_top = row![
        section_button(PidBrowserSection::Overview, active_section),
        section_button(PidBrowserSection::Objects, active_section),
        section_button(PidBrowserSection::Relationships, active_section),
    ]
    .spacing(6);

    let sections_bottom = row![
        section_button(PidBrowserSection::Sheets, active_section),
        section_button(PidBrowserSection::Streams, active_section),
        section_button(PidBrowserSection::CrossRef, active_section),
    ]
    .spacing(6);

    let search = text_input("Search P&ID", search_text_value)
        .on_input(Message::PidSearchChanged)
        .padding([6, 8])
        .size(12);

    let body: Element<'a, Message> = if items.is_empty() {
        container(text(empty_hint).size(11).color(HINT))
            .padding([10, 8])
            .width(Length::Fill)
            .into()
    } else {
        let mut list = column![].spacing(6);
        for item in items {
            let selected = selected_key.as_ref() == Some(&item.key);
            let mut lines =
                column![text(item.title.clone()).size(12).color(Color::WHITE)].spacing(2);
            if let Some(subtitle) = item.subtitle.clone() {
                lines = lines.push(text(subtitle).size(10).color(SUBTEXT));
            }
            if let Some(badge) = item.badge.clone() {
                lines = lines.push(text(badge).size(10).color(BADGE));
            }

            let button = button(container(lines).width(Length::Fill).padding([8, 8]))
                .width(Length::Fill)
                .on_press(Message::PidBrowserSelect(item.key))
                .style(move |theme: &Theme, status| item_button_style(theme, status, selected));

            list = list.push(button);
        }
        scrollable(list).into()
    };

    container(
        column![
            header,
            container(column![sections_top, sections_bottom, search].spacing(8))
                .padding([8, 8]),
            body
        ]
        .spacing(0),
    )
    .style(|_: &Theme| container::Style {
        background: Some(Background::Color(PANEL_BG)),
        border: Border {
            color: BORDER,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    })
    .width(280)
    .height(Length::Fill)
    .into()
}

fn section_button(section: PidBrowserSection, active_section: PidBrowserSection) -> iced::widget::Button<'static, Message> {
    let active = section == active_section;
    button(text(section.label()).size(11))
        .on_press(Message::PidBrowserSectionSelect(section))
        .style(move |theme: &Theme, status| section_button_style(theme, status, active))
}

fn section_button_style(theme: &Theme, status: button::Status, active: bool) -> button::Style {
    let palette = theme.extended_palette();
    let mut style = button::secondary(theme, status);
    style.text_color = if active {
        Color::WHITE
    } else {
        palette.secondary.base.text
    };
    style.background = Some(Background::Color(if active {
        Color::from_rgb(0.16, 0.34, 0.56)
    } else {
        Color::from_rgb(0.18, 0.19, 0.22)
    }));
    style.border = Border {
        color: BORDER,
        width: 1.0,
        radius: 3.0.into(),
    };
    style
}

fn item_button_style(theme: &Theme, status: button::Status, selected: bool) -> button::Style {
    let mut style = button::secondary(theme, status);
    style.background = Some(Background::Color(if selected {
        Color::from_rgb(0.14, 0.24, 0.40)
    } else {
        Color::from_rgb(0.16, 0.17, 0.20)
    }));
    style.border = Border {
        color: if selected {
            Color::from_rgb(0.35, 0.58, 0.86)
        } else {
            BORDER
        },
        width: 1.0,
        radius: 4.0.into(),
    };
    style.text_color = Color::WHITE;
    style
}

const PANEL_BG: Color = Color::from_rgb(0.11, 0.12, 0.14);
const HEADER_BG: Color = Color::from_rgb(0.15, 0.16, 0.18);
const BORDER: Color = Color::from_rgb(0.25, 0.26, 0.30);
const HINT: Color = Color::from_rgb(0.63, 0.66, 0.72);
const SUBTEXT: Color = Color::from_rgb(0.73, 0.75, 0.79);
const BADGE: Color = Color::from_rgb(0.44, 0.78, 0.78);
