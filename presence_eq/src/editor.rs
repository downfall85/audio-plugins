use nih_plug::prelude::*;
use nih_plug_iced::widgets as nih_widgets;
use nih_plug_iced::*;
use std::sync::Arc;

use crate::PresenceEqParams;

pub fn default_state() -> Arc<IcedState> {
    IcedState::from_size(340, 260)
}

pub fn create(
    params: Arc<PresenceEqParams>,
    editor_state: Arc<IcedState>,
) -> Option<Box<dyn Editor>> {
    create_iced_editor::<PresenceEqEditor>(editor_state, params)
}

struct PresenceEqEditor {
    params: Arc<PresenceEqParams>,
    context: Arc<dyn GuiContext>,

    hp_freq_state: nih_widgets::param_slider::State,
    lp_freq_state: nih_widgets::param_slider::State,
    mid_freq_state: nih_widgets::param_slider::State,
    mid_gain_state: nih_widgets::param_slider::State,
    mid_q_state: nih_widgets::param_slider::State,
    output_gain_state: nih_widgets::param_slider::State,
}

#[derive(Debug, Clone, Copy)]
enum Message {
    ParamUpdate(nih_widgets::ParamMessage),
}

impl IcedEditor for PresenceEqEditor {
    type Executor = executor::Default;
    type Message = Message;
    type InitializationFlags = Arc<PresenceEqParams>;

    fn new(
        params: Self::InitializationFlags,
        context: Arc<dyn GuiContext>,
    ) -> (Self, Command<Self::Message>) {
        let editor = PresenceEqEditor {
            params,
            context,
            hp_freq_state: Default::default(),
            lp_freq_state: Default::default(),
            mid_freq_state: Default::default(),
            mid_gain_state: Default::default(),
            mid_q_state: Default::default(),
            output_gain_state: Default::default(),
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
        let label_width = Length::Units(100);

        macro_rules! row {
            ($label:expr, $state:expr, $param:expr) => {
                Row::new()
                    .push(
                        Text::new($label)
                            .width(label_width)
                            .horizontal_alignment(alignment::Horizontal::Right),
                    )
                    .push(
                        nih_widgets::ParamSlider::new($state, $param).map(Message::ParamUpdate),
                    )
            };
        }

        Column::new()
            .align_items(Alignment::Center)
            .push(
                Text::new("Presence EQ")
                    .size(24)
                    .width(Length::Fill)
                    .horizontal_alignment(alignment::Horizontal::Center),
            )
            .push(row!("HP Freq", &mut self.hp_freq_state, &self.params.hp_freq))
            .push(row!("LP Freq", &mut self.lp_freq_state, &self.params.lp_freq))
            .push(row!("Mid Freq", &mut self.mid_freq_state, &self.params.mid_freq))
            .push(row!("Mid Gain", &mut self.mid_gain_state, &self.params.mid_gain))
            .push(row!("Mid Q", &mut self.mid_q_state, &self.params.mid_q))
            .push(row!("Output Gain", &mut self.output_gain_state, &self.params.output_gain))
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
