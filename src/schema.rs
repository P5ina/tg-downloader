use teloxide::{
    dispatching::{
        UpdateHandler,
        dialogue::{self, InMemStorage},
    },
    prelude::*,
    utils::command::BotCommands,
};

use crate::{
    commands::*,
    errors::BotError,
    handlers::{duplicated_link_received, format_received, link_received, quality_received, video_received},
    utils::is_youtube_video_link,
};

pub type MyDialogue = Dialogue<State, InMemStorage<State>>;

#[derive(Clone, Default)]
pub enum State {
    #[default]
    Start,
    ReceiveQuality {
        url: String,
    },
    ReceiveFormat {
        filename: String,
    },
}

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase")]
enum Command {
    /// Show start menu
    Start,
    /// Cancel the download.
    Cancel,
}

pub fn schema() -> UpdateHandler<BotError> {
    use dptree::case;

    dialogue::enter::<Update, InMemStorage<State>, State, _>()
        .branch(
            // Filter for messages
            Update::filter_message()
                .branch(
                    // Filter for commands
                    teloxide::filter_command::<Command, _>()
                        .branch(case![State::Start].branch(case![Command::Start].endpoint(start)))
                        .branch(case![Command::Cancel].endpoint(cancel)), // .branch(case![Command::Cancel].endpoint(cancel)),
                )
                // Filter for the youtube links
                .branch(
                    Message::filter_text()
                        .filter(|text: String| is_youtube_video_link(&text))
                        .branch(case![State::Start].endpoint(link_received))
                        .branch(
                            case![State::ReceiveQuality { url }]
                                .endpoint(duplicated_link_received),
                        )
                        .branch(
                            case![State::ReceiveFormat { filename }]
                                .endpoint(duplicated_link_received),
                        ),
                )
                .branch(
                    Message::filter_video()
                        .filter(|msg: Message| {
                            // Skip if message contains YouTube link (it's just a preview)
                            msg.text()
                                .map(|t| !is_youtube_video_link(t))
                                .unwrap_or(true)
                        })
                        .endpoint(video_received),
                ),
        )
        .branch(
            Update::filter_callback_query()
                .branch(case![State::ReceiveQuality { url }].endpoint(quality_received))
                .branch(case![State::ReceiveFormat { filename }].endpoint(format_received)),
        )
}
