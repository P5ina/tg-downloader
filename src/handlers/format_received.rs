use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use teloxide::{
    ApiError, RequestError,
    prelude::*,
    types::{InputFile, MaybeInaccessibleMessage, ParseMode},
};
use tokio::fs;
use tokio::sync::mpsc;

use crate::{
    errors::{BotError, ConversionError, HandlerResult},
    schema::MyDialogue,
    utils::{
        MediaFormatType, compression_loading_screen_with_progress, loading_screen_with_progress,
    },
    video::convert::{convert_audio, convert_video_note},
    video::{VideoInfo, compress_video_with_progress},
};

pub async fn format_received(
    bot: Bot,
    dialogue: MyDialogue,
    filename: String,
    query: CallbackQuery,
) -> HandlerResult {
    if let Some(s) = &query.data {
        let message = query
            .message
            .ok_or_else(|| BotError::general("Couldn't find message"))?;
        let chat_id = match message {
            MaybeInaccessibleMessage::Inaccessible(ref m) => m.chat.id,
            MaybeInaccessibleMessage::Regular(ref m) => m.chat.id,
        };
        bot.answer_callback_query(&query.id).await?;

        let media_format = MediaFormatType::from_str(s)?;
        log::info!("Found media format {:?}", media_format);

        // Для видео формата сразу отправляем без конвертации
        if media_format == MediaFormatType::Video {
            let message_id = match message {
                MaybeInaccessibleMessage::Inaccessible(m) => {
                    let message = bot
                        .send_message(m.chat.id, "📤 Отправляем видео...")
                        .await?;
                    message.id
                }
                MaybeInaccessibleMessage::Regular(m) => {
                    bot.edit_message_text(chat_id, m.id, "📤 Отправляем видео...")
                        .await?;
                    m.id
                }
            };

            let video_info = VideoInfo::from_file(&filename).await?;
            let result = bot.send_video(chat_id, InputFile::file(&filename))
                .width(video_info.width)
                .height(video_info.height)
                .duration(video_info.duration as u32)
                .await;

            match result {
                Ok(_) => {
                    bot.edit_message_text(
                        chat_id,
                        message_id,
                        "✅ Готово! Ваше видео отправлено!",
                    )
                    .await?;
                    bot.send_message(
                        chat_id,
                        "Можете теперь отправить еще одно видео.",
                    )
                    .await?;
                }
                Err(RequestError::Api(ApiError::RequestEntityTooLarge)) => {
                    bot.edit_message_text(
                        chat_id,
                        message_id,
                        "❌ Ваше видео слишком большое, мы не можем его отправить.",
                    )
                    .await?;
                }
                Err(e) => {
                    fs::remove_file(&filename).await?;
                    return Err(e.into());
                }
            }

            dialogue
                .exit()
                .await
                .map_err(|e| BotError::general(format!("Failed to exit dialogue: {}", e)))?;

            // Cleanup
            fs::remove_file(filename).await?;
            return Ok(());
        }

        let message_id = match message {
            MaybeInaccessibleMessage::Inaccessible(m) => {
                let message = bot
                    .send_message(m.chat.id, "🚀 Начинаем конвертацию...")
                    .await?;
                message.id
            }
            MaybeInaccessibleMessage::Regular(m) => {
                bot.edit_message_text(chat_id, m.id, "🚀 Начинаем конвертацию...")
                    .await?;
                m.id
            }
        };

        // Запускаем loading screen
        let should_stop_loading = Arc::new(AtomicBool::new(false));
        let (_progress_tx, progress_rx) = mpsc::unbounded_channel();
        let loading_task = {
            let bot_clone = bot.clone();
            let should_stop_clone = should_stop_loading.clone();
            tokio::spawn(async move {
                loading_screen_with_progress(
                    bot_clone,
                    chat_id,
                    message_id,
                    should_stop_clone,
                    progress_rx,
                )
                .await;
            })
        };

        let formated_filename_result = match media_format {
            MediaFormatType::Video => {
                // Для видео формата просто используем оригинальный файл без конвертации
                Ok(filename.clone())
            }
            MediaFormatType::VideoNote => {
                bot.send_message(
                    chat_id,
                    "<b>⚠️ Внимание</b> кружочек будет обрезан до 1 минуты.",
                )
                .parse_mode(ParseMode::Html)
                .await?;
                convert_video_note(&filename).await
            }
            MediaFormatType::Audio | MediaFormatType::Voice => convert_audio(&filename).await,
        };

        let formated_filename = match formated_filename_result {
            Ok(f) => f,
            Err(BotError::ConversionError(e)) => {
                match e {
                    ConversionError::NonUtf8Path | ConversionError::IOError(_) => {
                        // Останавливаем loading screen
                        should_stop_loading.store(true, Ordering::Relaxed);
                        loading_task.abort();

                        fs::remove_file(filename).await?;
                        return Err(BotError::ConversionError(e));
                    }
                    ConversionError::FfmpegFailed(exit, stderr) => {
                        log::error!("Ffmpeg error: Exit code {}, output: {}", exit, stderr);

                        // Останавливаем loading screen
                        should_stop_loading.store(true, Ordering::Relaxed);
                        loading_task.abort();

                        fs::remove_file(filename).await?;
                        bot.edit_message_text(chat_id, message_id,
                        "❌ Мы не смогли конвертировать ваше видео, попробуйте выбрать другой формат. \
                            Или попробуйте загрузить другое видео использовав команду /cancel").await?;
                        return Ok(());
                    }
                }
            }
            Err(BotError::FileTooLarge(_)) if media_format == MediaFormatType::Video => {
                // Only try compression for Video format

                // Останавливаем основной loading screen
                should_stop_loading.store(true, Ordering::Relaxed);
                loading_task.abort();

                // Показываем начальное сообщение о сжатии
                bot.edit_message_text(
                    chat_id,
                    message_id,
                    "🔧 Видео получилось слишком большим (>200МБ), начинаем сжатие...",
                )
                .await?;

                // Запускаем loading screen для сжатия
                let should_stop_compression = Arc::new(AtomicBool::new(false));
                let (compression_progress_tx, compression_progress_rx) = mpsc::unbounded_channel();
                let compression_task = {
                    let bot_clone = bot.clone();
                    let should_stop_clone = should_stop_compression.clone();
                    tokio::spawn(async move {
                        compression_loading_screen_with_progress(
                            bot_clone,
                            chat_id,
                            message_id,
                            should_stop_clone,
                            compression_progress_rx,
                        )
                        .await;
                    })
                };

                match compress_video_with_progress(&filename, Some(compression_progress_tx)).await {
                    Ok(compressed_file) => {
                        // Останавливаем compression loading screen
                        should_stop_compression.store(true, Ordering::Relaxed);
                        compression_task.abort();

                        bot.edit_message_text(
                            chat_id,
                            message_id,
                            "✅ Видео успешно сжато до допустимого размера!",
                        )
                        .await?;
                        compressed_file
                    }
                    Err(BotError::FileTooLarge(_)) => {
                        // Останавливаем compression loading screen
                        should_stop_compression.store(true, Ordering::Relaxed);
                        compression_task.abort();

                        fs::remove_file(filename).await?;
                        bot.edit_message_text(
                            chat_id,
                            message_id,
                            "❌ К сожалению, не удалось сжать видео до 200МБ. \
                            Попробуйте загрузить видео меньшего размера или более низкого качества."
                        ).await?;
                        return Ok(());
                    }
                    Err(e) => {
                        // Останавливаем compression loading screen
                        should_stop_compression.store(true, Ordering::Relaxed);
                        compression_task.abort();

                        fs::remove_file(filename).await?;
                        return Err(e);
                    }
                }
            }
            Err(e) => {
                // Останавливаем loading screen
                should_stop_loading.store(true, Ordering::Relaxed);
                loading_task.abort();

                fs::remove_file(filename).await?;
                return Err(e);
            }
        };

        let result = match media_format {
            MediaFormatType::Video => {
                let video_info = VideoInfo::from_file(&formated_filename).await?;
                bot.send_video(chat_id, InputFile::file(&formated_filename))
                    .width(video_info.width)
                    .height(video_info.height)
                    .duration(video_info.duration as u32)
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

        // Останавливаем loading screen
        should_stop_loading.store(true, Ordering::Relaxed);
        loading_task.abort(); // Принудительно завершаем задачу

        match result {
            Ok(_) => {
                bot.edit_message_text(
                    chat_id,
                    message_id,
                    "✅ Готово! Ваше видео успешно конвертировано!",
                )
                .await?;
                bot.send_message(
                    chat_id,
                    "Можете теперь отправить еще одно видео, чтобы сконвертировать и его.",
                )
                .await?;
            }
            Err(RequestError::Api(ApiError::RequestEntityTooLarge)) => {
                bot.edit_message_text(
                    chat_id,
                    message_id,
                    "❌ Ваше видео получилось слишком большим, мы не можем его отправить.",
                )
                .await?;
            }
            Err(e) => return Err(e.into()),
        }
        dialogue
            .exit()
            .await
            .map_err(|e| BotError::general(format!("Failed to exit dialogue: {}", e)))?;

        // Cleanup
        fs::remove_file(formated_filename).await?;
        fs::remove_file(filename).await?;
    }

    Ok(())
}
