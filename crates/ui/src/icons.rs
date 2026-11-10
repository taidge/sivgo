//! Shared vector icons for editor actions. Accessible names belong to the buttons.
use glory::{
    render::{GraphicPoint, GraphicProps, GraphicStroke, PathCommand, StrokeCap},
    style::{Length, PointerEvents, Style},
    ui::{graphic, Graphic},
};

#[derive(Clone, Copy)]
pub enum EditorIcon {
    New,
    Open,
    Save,
    Export,
    Undo,
    Redo,
    ZoomIn,
    ZoomOut,
    Fit,
    Agent,
    Send,
    Settings,
}

impl EditorIcon {
    pub fn widget(self) -> Graphic {
        let lines: &[&[(f64, f64)]] = match self {
            Self::New => &[
                &[
                    (12., 21.),
                    (5., 21.),
                    (5., 3.),
                    (14., 3.),
                    (19., 8.),
                    (19., 12.),
                ],
                &[(14., 3.), (14., 8.), (19., 8.)],
                &[(18., 14.), (18., 22.)],
                &[(14., 18.), (22., 18.)],
            ],
            Self::Open => &[
                &[
                    (3., 19.),
                    (3., 5.),
                    (9., 5.),
                    (11., 8.),
                    (19., 8.),
                    (19., 11.),
                ],
                &[(3., 19.), (6., 11.), (22., 11.), (19., 19.), (3., 19.)],
            ],
            Self::Save => &[
                &[
                    (4., 3.),
                    (17., 3.),
                    (21., 7.),
                    (21., 21.),
                    (3., 21.),
                    (3., 3.),
                    (4., 3.),
                ],
                &[(7., 3.), (7., 9.), (16., 9.), (16., 3.)],
                &[(7., 21.), (7., 14.), (17., 14.), (17., 21.)],
            ],
            Self::Export => &[
                &[(3., 14.), (3., 20.), (21., 20.), (21., 14.)],
                &[(12., 3.), (12., 15.)],
                &[(7., 10.), (12., 15.), (17., 10.)],
            ],
            Self::Undo | Self::Redo => &[
                &[(9., 4.), (3., 10.), (9., 16.)],
                &[(3., 10.), (14., 10.), (19., 12.), (21., 16.), (21., 20.)],
            ],
            Self::ZoomIn => &[&[(5., 12.), (19., 12.)], &[(12., 5.), (12., 19.)]],
            Self::ZoomOut => &[&[(5., 12.), (19., 12.)]],
            Self::Fit => &[
                &[(4., 9.), (4., 4.), (9., 4.)],
                &[(15., 4.), (20., 4.), (20., 9.)],
                &[(20., 15.), (20., 20.), (15., 20.)],
                &[(9., 20.), (4., 20.), (4., 15.)],
            ],
            Self::Agent => &[
                &[
                    (10., 3.),
                    (12.5, 9.5),
                    (19., 12.),
                    (12.5, 14.5),
                    (10., 21.),
                    (7.5, 14.5),
                    (1., 12.),
                    (7.5, 9.5),
                    (10., 3.),
                ],
                &[(20., 3.), (20., 9.)],
                &[(17., 6.), (23., 6.)],
            ],
            Self::Send => &[
                &[(3., 4.), (21., 12.), (3., 20.), (6., 12.), (3., 4.)],
                &[(6., 12.), (15., 12.)],
            ],
            Self::Settings => &[
                &[(4., 6.), (20., 6.)],
                &[(4., 12.), (20., 12.)],
                &[(4., 18.), (20., 18.)],
                &[(9., 4.), (9., 8.)],
                &[(15., 10.), (15., 14.)],
                &[(8., 16.), (8., 20.)],
            ],
        };
        let commands = lines
            .iter()
            .flat_map(|line| {
                line.iter().enumerate().map(move |(index, &(x, y))| {
                    let x = if matches!(self, Self::Redo) {
                        24. - x
                    } else {
                        x
                    };
                    let point = GraphicPoint::new(x, y);
                    if index == 0 {
                        PathCommand::Move(point)
                    } else {
                        PathCommand::Line(point)
                    }
                })
            })
            .collect();
        graphic()
            .drawing(GraphicProps {
                commands,
                stroke: Some(GraphicStroke {
                    color: sapid::Theme::dark().foreground,
                    width: 1.8,
                    cap: StrokeCap::Round,
                }),
                ..Default::default()
            })
            .style(
                Style::new()
                    .width(Length::Px(16.))
                    .height(Length::Px(16.))
                    .pointer_events(PointerEvents::None),
            )
    }
}
