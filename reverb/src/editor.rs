use nih_plug::prelude::*;
use nih_plug_iced::widgets as nih_widgets;
use nih_plug_iced::*;
use std::sync::Arc;

use crate::ReverbParams;

pub fn default_state() -> Arc<IcedState> {
    IcedState::from_size(300, 180)
}

pub fn create(
    params: Arc<ReverbParams>,
    editor_state: Arc<IcedState>,
) -> Option<Box<dyn Editor>> {
    create_iced_editor::<ReverbEditor>(editor_state, params)
}

struct ReverbEditor {
    params: Arc<ReverbParams>,
    context: Arc<dyn GuiContext>,

    room_size_slider_state: nih_widgets::param_slider::State,
    damping_slider_state: nih_widgets::param_slider::State,
    wet_slider_state: nih_widgets::param_slider::State,
}

#[derive(Debug, Clone, Copy)]
enum Message {
    ParamUpdate(nih_widgets::ParamMessage),
}

impl IcedEditor for ReverbEditor {
    type Executor = executor::Default;
    type Message = Message;
    type InitializationFlags = Arc<ReverbParams>;

    fn new(
        params: Self::InitializationFlags,
        context: Arc<dyn GuiContext>,
    ) -> (Self, Command<Self::Message>) {
        let editor = ReverbEditor {
            params,
            context,
            room_size_slider_state: Default::default(),
            damping_slider_state: Default::default(),
            wet_slider_state: Default::default(),
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
        }
        Command::none()
    }

    fn view(&mut self) -> Element<'_, Self::Message> {
        Column::new()
            .align_items(Alignment::Center)
            .push(
                Text::new("Reverb")
                    .size(24)
                    .width(Length::Fill)
                    .horizontal_alignment(alignment::Horizontal::Center),
            )
            .push(
                Row::new()
                    .push(
                        Text::new("Room Size")
                            .width(Length::Units(100))
                            .horizontal_alignment(alignment::Horizontal::Right),
                    )
                    .push(
                        nih_widgets::ParamSlider::new(
                            &mut self.room_size_slider_state,
                            &self.params.room_size,
                        )
                        .map(Message::ParamUpdate),
                    ),
            )
            .push(
                Row::new()
                    .push(
                        Text::new("Damping")
                            .width(Length::Units(100))
                            .horizontal_alignment(alignment::Horizontal::Right),
                    )
                    .push(
                        nih_widgets::ParamSlider::new(
                            &mut self.damping_slider_state,
                            &self.params.damping,
                        )
                        .map(Message::ParamUpdate),
                    ),
            )
            .push(
                Row::new()
                    .push(
                        Text::new("Wet")
                            .width(Length::Units(100))
                            .horizontal_alignment(alignment::Horizontal::Right),
                    )
                    .push(
                        nih_widgets::ParamSlider::new(
                            &mut self.wet_slider_state,
                            &self.params.wet,
                        )
                        .map(Message::ParamUpdate),
                    ),
            )
            .into()
    }

    fn background_color(&self) -> nih_plug_iced::Color {
        nih_plug_iced::Color {
            r: 0.15,
            g: 0.15,
            b: 0.15,
            a: 1.0,
        }
    }
}
