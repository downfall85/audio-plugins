use nih_plug::prelude::*;
use nih_plug_iced::widgets as nih_widgets;
use nih_plug_iced::*;
use std::sync::Arc;

use crate::NoiseGateParams;

pub fn default_state() -> Arc<IcedState> {
    IcedState::from_size(300, 180)
}

pub fn create(
    params: Arc<NoiseGateParams>,
    editor_state: Arc<IcedState>,
) -> Option<Box<dyn Editor>> {
    create_iced_editor::<NoiseGateEditor>(editor_state, params)
}

struct NoiseGateEditor {
    params: Arc<NoiseGateParams>,
    context: Arc<dyn GuiContext>,

    threshold_slider_state: nih_widgets::param_slider::State,
    attack_slider_state: nih_widgets::param_slider::State,
    release_slider_state: nih_widgets::param_slider::State,
}

#[derive(Debug, Clone, Copy)]
enum Message {
    ParamUpdate(nih_widgets::ParamMessage),
}

impl IcedEditor for NoiseGateEditor {
    type Executor = executor::Default;
    type Message = Message;
    type InitializationFlags = Arc<NoiseGateParams>;

    fn new(
        params: Self::InitializationFlags,
        context: Arc<dyn GuiContext>,
    ) -> (Self, Command<Self::Message>) {
        let editor = NoiseGateEditor {
            params,
            context,
            threshold_slider_state: Default::default(),
            attack_slider_state: Default::default(),
            release_slider_state: Default::default(),
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
                Text::new("Noise Gate")
                    .size(24)
                    .width(Length::Fill)
                    .horizontal_alignment(alignment::Horizontal::Center),
            )
            .push(
                Row::new()
                    .push(
                        Text::new("Threshold")
                            .width(Length::Units(100))
                            .horizontal_alignment(alignment::Horizontal::Right),
                    )
                    .push(
                        nih_widgets::ParamSlider::new(
                            &mut self.threshold_slider_state,
                            &self.params.threshold,
                        )
                        .map(Message::ParamUpdate),
                    ),
            )
            .push(
                Row::new()
                    .push(
                        Text::new("Attack")
                            .width(Length::Units(100))
                            .horizontal_alignment(alignment::Horizontal::Right),
                    )
                    .push(
                        nih_widgets::ParamSlider::new(
                            &mut self.attack_slider_state,
                            &self.params.attack_ms,
                        )
                        .map(Message::ParamUpdate),
                    ),
            )
            .push(
                Row::new()
                    .push(
                        Text::new("Release")
                            .width(Length::Units(100))
                            .horizontal_alignment(alignment::Horizontal::Right),
                    )
                    .push(
                        nih_widgets::ParamSlider::new(
                            &mut self.release_slider_state,
                            &self.params.release_ms,
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
