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
    handlers::{
        format_callback_received, format_first_received, format_received, handle_pre_checkout_query,
        handle_successful_payment, link_received, quality_received, video_received,
    },
    utils::is_youtube_video_link,
};

pub type MyDialogue = Dialogue<State, InMemStorage<State>>;

#[derive(Clone, Default)]
pub enum State {
    #[default]
    Start,
    /// Legacy state for direct video upload format selection
    ReceiveFormat { filename: String },
}

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase")]
enum Command {
    /// Show start menu
    Start,
    /// Cancel the download
    Cancel,
    /// Show queue status
    Queue,
    /// Show premium subscription status
    Premium,
    /// Grant subscription (admin only)
    Grant,
}

/// Check if callback data is a format selection from queue (fmt:...)
fn is_format_callback(data: &str) -> bool {
    data.starts_with("fmt:")
}

/// Check if callback data is a format first selection (ff:...)
fn is_format_first_callback(data: &str) -> bool {
    data.starts_with("ff:")
}

/// Check if callback data is a quality selection (q:...)
fn is_quality_callback(data: &str) -> bool {
    data.starts_with("q:")
}

/// Check if callback data is a buy premium action
fn is_buy_premium_callback(data: &str) -> bool {
    data == "buy_premium"
}

pub fn schema() -> UpdateHandler<BotError> {
    use dptree::case;

    dptree::entry()
        // Handle pre-checkout queries (outside of dialogue)
        .branch(Update::filter_pre_checkout_query().endpoint(handle_pre_checkout_query))
        // Everything else goes through dialogue
        .branch(
            dialogue::enter::<Update, InMemStorage<State>, State, _>()
                .branch(
                    // Filter for messages
                    Update::filter_message()
                        .branch(
                            // Filter for commands
                            teloxide::filter_command::<Command, _>()
                                .branch(case![Command::Start].endpoint(start))
                                .branch(case![Command::Cancel].endpoint(cancel))
                                .branch(case![Command::Queue].endpoint(queue))
                                .branch(case![Command::Premium].endpoint(premium))
                                .branch(case![Command::Grant].endpoint(grant)),
                        )
                        // Filter for the youtube links - now accepts links in any state
                        .branch(
                            Message::filter_text()
                                .filter(|text: String| is_youtube_video_link(&text))
                                .endpoint(link_received),
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
                        )
                        // Handle successful payment
                        .branch(
                            dptree::filter(|msg: Message| msg.successful_payment().is_some())
                                .endpoint(handle_successful_payment),
                        ),
                )
                .branch(
                    Update::filter_callback_query()
                        // Handle buy premium callback
                        .branch(
                            dptree::filter(|q: CallbackQuery| {
                                q.data
                                    .as_ref()
                                    .map(|d| is_buy_premium_callback(d))
                                    .unwrap_or(false)
                            })
                            .endpoint(handle_buy_premium_callback),
                        )
                        // Handle format first selection (ff:format_index:short_id)
                        .branch(
                            dptree::filter(|q: CallbackQuery| {
                                q.data
                                    .as_ref()
                                    .map(|d| is_format_first_callback(d))
                                    .unwrap_or(false)
                            })
                            .endpoint(format_first_received),
                        )
                        // Handle quality selection from queue (q:short_id:height)
                        .branch(
                            dptree::filter(|q: CallbackQuery| {
                                q.data
                                    .as_ref()
                                    .map(|d| is_quality_callback(d))
                                    .unwrap_or(false)
                            })
                            .endpoint(quality_received),
                        )
                        // Handle format selection from queue (fmt:format_index:short_id) - legacy for direct uploads
                        .branch(
                            dptree::filter(|q: CallbackQuery| {
                                q.data
                                    .as_ref()
                                    .map(|d| is_format_callback(d))
                                    .unwrap_or(false)
                            })
                            .endpoint(format_callback_received),
                        )
                        // Legacy handler for direct video upload format selection
                        .branch(case![State::ReceiveFormat { filename }].endpoint(format_received)),
                ),
        )
}
