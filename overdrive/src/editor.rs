use nih_plug::prelude::*;
use nih_plug_iced::widgets as nih_widgets;
use nih_plug_iced::*;
use std::sync::Arc;

use crate::OverdriveParams;

pub fn default_state() -> Arc<IcedState> {
    IcedState::from_size(300, 200)
}

pub fn create(
    params: Arc<OverdriveParams>,
    editor_state: Arc<IcedState>,
) -> Option<Box<dyn Editor>> {
    create_iced_editor::<OverdriveEditor>(editor_state, params)
}

struct OverdriveEditor {
    params: Arc<OverdriveParams>,
    context: Arc<dyn GuiContext>,

    drive_slider_state: nih_widgets::param_slider::State,
    tone_slider_state: nih_widgets::param_slider::State,
    output_slider_state: nih_widgets::param_slider::State,
    mix_slider_state: nih_widgets::param_slider::State,
}

#[derive(Debug, Clone, Copy)]
enum Message {
    ParamUpdate(nih_widgets::ParamMessage),
}

impl IcedEditor for OverdriveEditor {
    type Executor = executor::Default;
    type Message = Message;
    type InitializationFlags = Arc<OverdriveParams>;

    fn new(
        params: Self::InitializationFlags,
        context: Arc<dyn GuiContext>,
    ) -> (Self, Command<Self::Message>) {
        let editor = OverdriveEditor {
            params,
            context,
            drive_slider_state: Default::default(),
            tone_slider_state: Default::default(),
            output_slider_state: Default::default(),
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
                Text::new("Overdrive")
                    .size(24)
                    .width(Length::Fill)
                    .horizontal_alignment(alignment::Horizontal::Center),
            )
            .push(
                Row::new()
                    .push(
                        Text::new("Drive")
                            .width(Length::Units(80))
                            .horizontal_alignment(alignment::Horizontal::Right),
                    )
                    .push(
                        nih_widgets::ParamSlider::new(
                            &mut self.drive_slider_state,
                            &self.params.drive,
                        )
                        .map(Message::ParamUpdate),
                    ),
            )
            .push(
                Row::new()
                    .push(
                        Text::new("Tone")
                            .width(Length::Units(80))
                            .horizontal_alignment(alignment::Horizontal::Right),
                    )
                    .push(
                        nih_widgets::ParamSlider::new(
                            &mut self.tone_slider_state,
                            &self.params.tone,
                        )
                        .map(Message::ParamUpdate),
                    ),
            )
            .push(
                Row::new()
                    .push(
                        Text::new("Output")
                            .width(Length::Units(80))
                            .horizontal_alignment(alignment::Horizontal::Right),
                    )
                    .push(
                        nih_widgets::ParamSlider::new(
                            &mut self.output_slider_state,
                            &self.params.output,
                        )
                        .map(Message::ParamUpdate),
                    ),
            )
            .push(
                Row::new()
                    .push(
                        Text::new("Mix")
                            .width(Length::Units(80))
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
            r: 0.18,
            g: 0.12,
            b: 0.08,
            a: 1.0,
        }
    }
}
