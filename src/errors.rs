use std::fmt;

/// Централизованная система ошибок для telegram бота
#[derive(Debug)]
pub enum BotError {
    /// Ошибки конвертации видео
    ConversionError(ConversionError),
    /// Ошибки работы с YouTube
    YoutubeError(String),
    /// Ошибки файловой системы
    FileSystemError(std::io::Error),
    /// Ошибки Telegram API
    TelegramError(teloxide::RequestError),
    /// Ошибки парсинга данных
    ParseError(String),
    /// Файл не найден
    FileNotFound(String),
    /// Неподдерживаемый формат
    UnsupportedFormat(String),
    /// Файл слишком большой
    FileTooLarge(String),
    /// Неверные параметры
    InvalidParameters(String),
    /// Внешняя команда завершилась с ошибкой
    ExternalCommandError { command: String, stderr: String },
    /// Общая ошибка с описанием
    General(String),
}

#[derive(Debug)]
pub enum ConversionError {
    NonUtf8Path,
    IOError(std::io::Error),
    FfmpegFailed(std::process::ExitStatus, String),
}

impl fmt::Display for BotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BotError::ConversionError(e) => write!(f, "Ошибка конвертации: {}", e),
            BotError::YoutubeError(msg) => write!(f, "Ошибка загрузки с YouTube: {}", msg),
            BotError::FileSystemError(e) => write!(f, "Ошибка файловой системы: {}", e),
            BotError::TelegramError(e) => write!(f, "Ошибка Telegram API: {}", e),
            BotError::ParseError(msg) => write!(f, "Ошибка парсинга: {}", msg),
            BotError::FileNotFound(path) => write!(f, "Файл не найден: {}", path),
            BotError::UnsupportedFormat(format) => write!(f, "Неподдерживаемый формат: {}", format),
            BotError::FileTooLarge(msg) => write!(f, "Файл слишком большой: {}", msg),
            BotError::InvalidParameters(msg) => write!(f, "Неверные параметры: {}", msg),
            BotError::ExternalCommandError { command, stderr } => {
                write!(f, "Ошибка команды {}: {}", command, stderr)
            }
            BotError::General(msg) => write!(f, "{}", msg),
        }
    }
}

impl fmt::Display for ConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConversionError::NonUtf8Path => write!(f, "Путь содержит недопустимые символы"),
            ConversionError::IOError(e) => write!(f, "Ошибка ввода-вывода: {}", e),
            ConversionError::FfmpegFailed(code, stderr) => {
                write!(f, "FFmpeg завершился с кодом {} - stderr: {}", code, stderr)
            }
        }
    }
}

impl std::error::Error for BotError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            BotError::ConversionError(e) => Some(e),
            BotError::FileSystemError(e) => Some(e),
            BotError::TelegramError(e) => Some(e),
            _ => None,
        }
    }
}

impl std::error::Error for ConversionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConversionError::IOError(e) => Some(e),
            _ => None,
        }
    }
}

// Реализации From для автоматического преобразования ошибок
impl From<ConversionError> for BotError {
    fn from(err: ConversionError) -> Self {
        BotError::ConversionError(err)
    }
}

impl From<std::io::Error> for BotError {
    fn from(err: std::io::Error) -> Self {
        BotError::FileSystemError(err)
    }
}

impl From<teloxide::RequestError> for BotError {
    fn from(err: teloxide::RequestError) -> Self {
        BotError::TelegramError(err)
    }
}

impl From<std::io::Error> for ConversionError {
    fn from(e: std::io::Error) -> Self {
        Self::IOError(e)
    }
}

impl From<serde_json::Error> for BotError {
    fn from(err: serde_json::Error) -> Self {
        BotError::ParseError(format!("JSON parsing error: {}", err))
    }
}

impl From<std::str::Utf8Error> for BotError {
    fn from(err: std::str::Utf8Error) -> Self {
        BotError::ParseError(format!("UTF-8 parsing error: {}", err))
    }
}

impl From<strum::ParseError> for BotError {
    fn from(err: strum::ParseError) -> Self {
        BotError::ParseError(format!("Enum parsing error: {}", err))
    }
}

// Удобные методы для создания ошибок
impl BotError {
    pub fn youtube_error(msg: impl Into<String>) -> Self {
        Self::YoutubeError(msg.into())
    }

    pub fn file_not_found(path: impl Into<String>) -> Self {
        Self::FileNotFound(path.into())
    }

    pub fn unsupported_format(format: impl Into<String>) -> Self {
        Self::UnsupportedFormat(format.into())
    }

    pub fn file_too_large(msg: impl Into<String>) -> Self {
        Self::FileTooLarge(msg.into())
    }

    pub fn invalid_parameters(msg: impl Into<String>) -> Self {
        Self::InvalidParameters(msg.into())
    }

    pub fn external_command_error(command: impl Into<String>, stderr: impl Into<String>) -> Self {
        Self::ExternalCommandError {
            command: command.into(),
            stderr: stderr.into(),
        }
    }

    pub fn general(msg: impl Into<String>) -> Self {
        Self::General(msg.into())
    }
}

/// Результат операций бота
pub type BotResult<T> = Result<T, BotError>;

/// Результат для хендлеров
pub type HandlerResult = BotResult<()>;
