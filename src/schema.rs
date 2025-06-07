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
    handlers::{format_received, link_received},
    utils::is_youtube_video_link,
};

pub type HandlerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;
pub type MyDialogue = Dialogue<State, InMemStorage<State>>;

#[derive(Clone, Default)]
pub enum State {
    #[default]
    Start,
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

pub fn schema() -> UpdateHandler<Box<dyn std::error::Error + Send + Sync + 'static>> {
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
                    dptree::filter(|msg: Message| {
                        msg.text().map_or(false, |url| is_youtube_video_link(url))
                    })
                    .endpoint(link_received),
                ),
        )
        .branch(
            Update::filter_callback_query()
                .branch(case![State::ReceiveFormat { filename }].endpoint(format_received)),
        )
}
