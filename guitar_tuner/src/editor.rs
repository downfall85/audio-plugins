use atomic_float::AtomicF32;
use nih_plug::prelude::*;
use nih_plug_iced::canvas::{self, Canvas, Frame, Geometry, Path, Stroke};
use nih_plug_iced::widgets as nih_widgets;
use nih_plug_iced::*;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use crate::{midi_to_note_name, GuitarTunerParams};

const NO_DETECTION: u8 = 255;

pub fn default_state() -> Arc<IcedState> {
    IcedState::from_size(350, 400)
}

pub fn create(
    params: Arc<GuitarTunerParams>,
    editor_state: Arc<IcedState>,
    detected_freq: Arc<AtomicF32>,
    detected_cents: Arc<AtomicF32>,
    detected_note: Arc<AtomicU8>,
) -> Option<Box<dyn Editor>> {
    create_iced_editor::<GuitarTunerEditor>(
        editor_state,
        (params, detected_freq, detected_cents, detected_note),
    )
}

struct GuitarTunerEditor {
    params: Arc<GuitarTunerParams>,
    context: Arc<dyn GuiContext>,

    detected_freq: Arc<AtomicF32>,
    detected_cents: Arc<AtomicF32>,
    detected_note: Arc<AtomicU8>,

    reference_pitch_slider_state: nih_widgets::param_slider::State,
}

#[derive(Debug, Clone, Copy)]
enum Message {
    ParamUpdate(nih_widgets::ParamMessage),
    Tick,
}

impl IcedEditor for GuitarTunerEditor {
    type Executor = executor::Default;
    type Message = Message;
    type InitializationFlags = (
        Arc<GuitarTunerParams>,
        Arc<AtomicF32>,
        Arc<AtomicF32>,
        Arc<AtomicU8>,
    );

    fn new(
        (params, detected_freq, detected_cents, detected_note): Self::InitializationFlags,
        context: Arc<dyn GuiContext>,
    ) -> (Self, Command<Self::Message>) {
        let editor = GuitarTunerEditor {
            params,
            context,
            detected_freq,
            detected_cents,
            detected_note,
            reference_pitch_slider_state: Default::default(),
        };
        (editor, Command::none())
    }

    fn context(&self) -> &dyn GuiContext {
        self.context.as_ref()
    }

    fn update(
        &mut self,
        _window: &mut WindowQueue,
        message: Self::Message,
    ) -> Command<Self::Message> {
        match message {
            Message::ParamUpdate(msg) => self.handle_param_message(msg),
            Message::Tick => {} // view() already reads fresh atomic values each call
        }
        Command::none()
    }

    fn subscription(
        &self,
        window_subs: &mut WindowSubs<Self::Message>,
    ) -> Subscription<Self::Message> {
        // Trigger a redraw on every display frame so the tuner stays live
        window_subs.on_frame = Some(Message::Tick);
        Subscription::none()
    }

    fn view(&mut self) -> Element<'_, Self::Message> {
        let note = self.detected_note.load(Ordering::Relaxed);
        let freq = self.detected_freq.load(Ordering::Relaxed);
        let cents = self.detected_cents.load(Ordering::Relaxed);

        let (note_str, freq_str) = if note == NO_DETECTION {
            ("--".to_string(), "-- Hz".to_string())
        } else {
            (
                midi_to_note_name(note as i32),
                format!("{:.1} Hz", freq),
            )
        };

        // Tuner bar drawn with Canvas (no cache — redrawn every frame)
        let tuner_bar = Canvas::new(TunerBar {
            cents,
            active: note != NO_DETECTION,
        })
        .width(Length::Units(310))
        .height(Length::Units(60));

        Column::new()
            .align_items(Alignment::Center)
            .padding(16)
            .spacing(8)
            .push(
                Text::new("Guitar Tuner")
                    .size(24)
                    .width(Length::Fill)
                    .horizontal_alignment(alignment::Horizontal::Center),
            )
            .push(
                Text::new(note_str)
                    .size(64)
                    .width(Length::Fill)
                    .horizontal_alignment(alignment::Horizontal::Center),
            )
            .push(
                Text::new(freq_str)
                    .size(16)
                    .width(Length::Fill)
                    .horizontal_alignment(alignment::Horizontal::Center),
            )
            .push(tuner_bar)
            .push(Space::with_height(Length::Units(4)))
            .push(
                Row::new()
                    .push(
                        Text::new("A4 Reference")
                            .width(Length::Units(110))
                            .horizontal_alignment(alignment::Horizontal::Right),
                    )
                    .push(
                        nih_widgets::ParamSlider::new(
                            &mut self.reference_pitch_slider_state,
                            &self.params.reference_pitch,
                        )
                        .map(Message::ParamUpdate),
                    ),
            )
            .push(Space::with_height(Length::Units(4)))
            .push(
                Text::new("E2   A2   D3   G3   B3   E4")
                    .size(12)
                    .width(Length::Fill)
                    .horizontal_alignment(alignment::Horizontal::Center),
            )
            .into()
    }

    fn background_color(&self) -> nih_plug_iced::Color {
        nih_plug_iced::Color {
            r: 0.12,
            g: 0.12,
            b: 0.12,
            a: 1.0,
        }
    }
}

// --- Canvas program for the tuner bar ---

struct TunerBar {
    cents: f32,
    active: bool,
}

impl canvas::Program<Message> for TunerBar {
    fn draw(&self, bounds: nih_plug_iced::Rectangle, _cursor: canvas::Cursor) -> Vec<Geometry> {
        let mut frame = Frame::new(bounds.size());

        let w = bounds.width;
        let h = bounds.height;
        let cx = w / 2.0;

        // Background
        frame.fill_rectangle(
            nih_plug_iced::Point::ORIGIN,
            bounds.size(),
            nih_plug_iced::Color {
                r: 0.08,
                g: 0.08,
                b: 0.08,
                a: 1.0,
            },
        );

        // Tick marks at -50, -25, 0, +25, +50 cents
        for &offset in &[-50.0f32, -25.0, 0.0, 25.0, 50.0] {
            let x = cx + offset / 50.0 * (w / 2.0 - 10.0);
            let tick_h = if offset == 0.0 { h * 0.6 } else { h * 0.35 };
            let tick_y = (h - tick_h) / 2.0;

            let color = if offset == 0.0 {
                nih_plug_iced::Color::WHITE
            } else {
                nih_plug_iced::Color {
                    r: 0.45,
                    g: 0.45,
                    b: 0.45,
                    a: 1.0,
                }
            };

            let tick_path = Path::line(
                nih_plug_iced::Point { x, y: tick_y },
                nih_plug_iced::Point { x, y: tick_y + tick_h },
            );
            frame.stroke(
                &tick_path,
                Stroke {
                    width: if offset == 0.0 { 2.0 } else { 1.0 },
                    color,
                    ..Stroke::default()
                },
            );
        }

        // Cent labels
        for (label, offset) in [("-50", -50.0f32), ("0", 0.0), ("+50", 50.0)] {
            let x = cx + offset / 50.0 * (w / 2.0 - 10.0);
            frame.fill_text(canvas::Text {
                content: label.to_string(),
                position: nih_plug_iced::Point { x, y: h - 13.0 },
                color: nih_plug_iced::Color {
                    r: 0.45,
                    g: 0.45,
                    b: 0.45,
                    a: 1.0,
                },
                size: 10.0,
                horizontal_alignment: alignment::Horizontal::Center,
                vertical_alignment: alignment::Vertical::Top,
                font: Default::default(),
            });
        }

        if self.active {
            // Colored indicator dot
            let clamped = self.cents.clamp(-50.0, 50.0);
            let indicator_x = cx + clamped / 50.0 * (w / 2.0 - 10.0);
            let indicator_y = h / 2.0;

            let indicator_color = if clamped.abs() < 5.0 {
                nih_plug_iced::Color { r: 0.1, g: 0.9, b: 0.3, a: 1.0 } // green
            } else if clamped.abs() < 15.0 {
                nih_plug_iced::Color { r: 0.95, g: 0.8, b: 0.1, a: 1.0 } // yellow
            } else {
                nih_plug_iced::Color { r: 0.9, g: 0.2, b: 0.1, a: 1.0 } // red
            };

            let dot = Path::circle(
                nih_plug_iced::Point { x: indicator_x, y: indicator_y },
                8.0,
            );
            frame.fill(&dot, indicator_color);
        }

        vec![frame.into_geometry()]
    }
}
