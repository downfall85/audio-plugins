use nih_plug::prelude::*;
use nih_plug_iced::widgets as nih_widgets;
use nih_plug_iced::*;
use std::sync::Arc;

use crate::DelayParams;

pub fn default_state() -> Arc<IcedState> {
    IcedState::from_size(300, 180)
}

pub fn create(
    params: Arc<DelayParams>,
    editor_state: Arc<IcedState>,
) -> Option<Box<dyn Editor>> {
    create_iced_editor::<DelayEditor>(editor_state, params)
}

struct DelayEditor {
    params: Arc<DelayParams>,
    context: Arc<dyn GuiContext>,

    time_slider_state: nih_widgets::param_slider::State,
    feedback_slider_state: nih_widgets::param_slider::State,
    mix_slider_state: nih_widgets::param_slider::State,
}

#[derive(Debug, Clone, Copy)]
enum Message {
    ParamUpdate(nih_widgets::ParamMessage),
}

impl IcedEditor for DelayEditor {
    type Executor = executor::Default;
    type Message = Message;
    type InitializationFlags = Arc<DelayParams>;

    fn new(
        params: Self::InitializationFlags,
        context: Arc<dyn GuiContext>,
    ) -> (Self, Command<Self::Message>) {
        let editor = DelayEditor {
            params,
            context,
            time_slider_state: Default::default(),
            feedback_slider_state: Default::default(),
            mix_slider_state: Default::default(),
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
                Text::new("Delay")
                    .size(24)
                    .width(Length::Fill)
                    .horizontal_alignment(alignment::Horizontal::Center),
            )
            .push(
                Row::new()
                    .push(
                        Text::new("Time (ms)")
                            .width(Length::Units(100))
                            .horizontal_alignment(alignment::Horizontal::Right),
                    )
                    .push(
                        nih_widgets::ParamSlider::new(
                            &mut self.time_slider_state,
                            &self.params.time_ms,
                        )
                        .map(Message::ParamUpdate),
                    ),
            )
            .push(
                Row::new()
                    .push(
                        Text::new("Feedback")
                            .width(Length::Units(100))
                            .horizontal_alignment(alignment::Horizontal::Right),
                    )
                    .push(
                        nih_widgets::ParamSlider::new(
                            &mut self.feedback_slider_state,
                            &self.params.feedback,
                        )
                        .map(Message::ParamUpdate),
                    ),
            )
            .push(
                Row::new()
                    .push(
                        Text::new("Mix")
                            .width(Length::Units(100))
                            .horizontal_alignment(alignment::Horizontal::Right),
                    )
                    .push(
                        nih_widgets::ParamSlider::new(
                            &mut self.mix_slider_state,
                            &self.params.mix,
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
