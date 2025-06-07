use std::str::FromStr;

use teloxide::{
    ApiError, RequestError,
    prelude::*,
    types::{InputFile, MaybeInaccessibleMessage, ParseMode},
};
use tokio::fs;

use crate::{
    convert::{ConversionError, convert_audio, convert_video, convert_video_note},
    schema::{HandlerResult, MyDialogue},
    utils::MediaFormatType,
};

pub async fn format_received(
    bot: Bot,
    dialogue: MyDialogue,
    filename: String,
    query: CallbackQuery,
) -> HandlerResult {
    if let Some(s) = &query.data {
        let message = query.message.ok_or("Couldn't find message")?;
        let chat_id = match message {
            MaybeInaccessibleMessage::Inaccessible(ref m) => m.chat.id,
            MaybeInaccessibleMessage::Regular(ref m) => m.chat.id,
        };
        bot.answer_callback_query(&query.id).await?;
        let message_id = match message {
            MaybeInaccessibleMessage::Inaccessible(m) => {
                let message = bot.send_message(m.chat.id, "Конвертируем...").await?;
                message.id
            }
            MaybeInaccessibleMessage::Regular(m) => {
                bot.edit_message_text(chat_id, m.id, "Конвертируем...")
                    .await?;
                m.id
            }
        };

        let media_format = MediaFormatType::from_str(s)?;
        log::info!("Found media format {:?}", media_format);

        let formated_filename_result = match media_format {
            MediaFormatType::Video => convert_video(&filename).await,
            MediaFormatType::VideoNote => {
                bot.edit_message_text(
                    chat_id,
                    message_id,
                    "Конвертируем...\n\n<b>Внимание</b> кружочек будет обрезан до 1 минуты.",
                )
                .parse_mode(ParseMode::Html)
                .await?;
                convert_video_note(&filename).await
            }
            MediaFormatType::Audio | MediaFormatType::Voice => convert_audio(&filename).await,
        };

        let formated_filename = match formated_filename_result {
            Ok(f) => f,
            Err(e) => {
                match e {
                    ConversionError::NonUtf8Path | ConversionError::IOError(_) => {
                        fs::remove_file(filename).await?;
                        return Err(Box::new(e));
                    }
                    ConversionError::FfmpegFailed(exit, stderr) => {
                        log::error!("Ffmpeg error: Exit code {exit}, output: {stderr}");
                        fs::remove_file(filename).await?;
                        bot.send_message(chat_id,
                        "Мы не смогли конвертировать ваше видео, попробуйте выбрать другой формат. \
                            Или попробуйте загрузить другое видео использовав команду /cancel").await?;
                        return Ok(());
                    }
                }
            }
        };

        let result = match media_format {
            MediaFormatType::Video => {
                bot.send_video(chat_id, InputFile::file(&formated_filename))
                    .await
            }
            MediaFormatType::Audio => {
                bot.send_audio(chat_id, InputFile::file(&formated_filename))
                    .await
            }
            MediaFormatType::VideoNote => {
                bot.send_video_note(chat_id, InputFile::file(&formated_filename))
                    .await
            }
            MediaFormatType::Voice => {
                bot.send_voice(chat_id, InputFile::file(&formated_filename))
                    .await
            }
        };

        match result {
            Ok(_) => {
                bot.send_message(
                    chat_id,
                    "Ваше видео готово! Можете теперь отправить еще одно видео, чтобы сконвертировать и его."
                ).await?;
            }
            Err(e) => match e {
                RequestError::Api(ref api_e) => match api_e {
                    ApiError::RequestEntityTooLarge => {
                        bot.send_message(
                            chat_id,
                            "Ваше видео получилось слишком большим, мы не можем его скачать.",
                        )
                        .await?;
                    }
                    _ => return Err(Box::new(e)),
                },
                _ => return Err(Box::new(e)),
            },
        }
        dialogue.exit().await?;

        // Cleanup
        fs::remove_file(formated_filename).await?;
        fs::remove_file(filename).await?;
    }

    Ok(())
}
