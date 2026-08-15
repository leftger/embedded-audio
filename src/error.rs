/// Errors from bank parsing and playback control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioError {
    InvalidBankMagic,
    UnsupportedBankVersion,
    EffectNotFound,
    InvalidEffectKind,
    TruncatedBank,
    NoBank,
    BankFull,
    VoiceBusy,
    PreviewIo,
    InvalidPayload,
}

impl AudioError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidBankMagic => "invalid bank magic",
            Self::UnsupportedBankVersion => "unsupported bank version",
            Self::EffectNotFound => "effect not found",
            Self::InvalidEffectKind => "invalid effect kind",
            Self::TruncatedBank => "truncated bank blob",
            Self::NoBank => "no sound bank loaded",
            Self::BankFull => "bank builder capacity exceeded",
            Self::VoiceBusy => "no free voice",
            Self::PreviewIo => "preview file I/O failed",
            Self::InvalidPayload => "invalid effect payload",
        }
    }
}
