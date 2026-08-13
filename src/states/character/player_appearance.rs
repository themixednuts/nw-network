//! Player cosmetic appearance replication.
//!
//! This is a single-group state: all fields are registered in group 0 in
//! declaration order. The compact icon payload duplicates the subset needed by
//! UI surfaces that render a small player portrait without reading every
//! detailed appearance field.

use crate::Marshaler;

/// Compact appearance data used by player icon and portrait displays.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Marshaler)]
pub struct PlayerAppearanceIconData {
    /// Gender selection.
    pub gender: u8,
    /// Race selection.
    pub race: u8,
    /// Skin-tone selection.
    pub skin_tone: u8,
    /// Hairstyle selection.
    pub hairstyle: u8,
    /// Hair-color selection.
    pub hair_color: u8,
    /// Facial-hair selection.
    pub facial_hair: u8,
    /// Facial-hair color selection.
    pub facial_hair_color: u8,
    /// Packed icon rendering flags.
    pub icon_flags: u16,
}

/// Full appearance snapshot used to update all replicated appearance fields.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlayerAppearanceSnapshot {
    /// Gender selection.
    pub gender: u8,
    /// Race selection.
    pub race: u8,
    /// Skin-tone selection.
    pub skin_tone: u8,
    /// Hairstyle selection.
    pub hairstyle: u8,
    /// Facial-hair selection.
    pub facial_hair: u8,
    /// Hair-color selection.
    pub hair_color: u8,
    /// Facial-hair color selection.
    pub facial_hair_color: u8,
    /// Eye-color selection.
    pub eye_color: u8,
    /// Face-mark selection.
    pub face_mark: u8,
    /// Scar selection.
    pub scar: u8,
    /// Tattoo selection.
    pub tattoo: u8,
    /// Tattoo-color selection.
    pub tattoo_color: u8,
    /// Version or dirty flag for appearance changes.
    pub appearance_change_flag: u8,
}

impl PlayerAppearanceSnapshot {
    #[must_use]
    pub const fn icon_data(self) -> PlayerAppearanceIconData {
        PlayerAppearanceIconData {
            gender: self.gender,
            race: self.race,
            skin_tone: self.skin_tone,
            hairstyle: self.hairstyle,
            hair_color: self.hair_color,
            facial_hair: self.facial_hair,
            facial_hair_color: self.facial_hair_color,
            icon_flags: 0,
        }
    }
}

/// Replicated player appearance fields.
///
/// The declaration order is the group 0 wire registration order. Most fields
/// are one-byte cosmetic indices; `icon_n_data` carries the compact portrait
/// subset and `appearance_change_flag` lets consumers notice applied changes.
pub use crate::generated::states::PlayerAppearanceComponentReplicatedState;
