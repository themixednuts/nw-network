//! Player cosmetic appearance replication.
//!
//! This is a single-group state: all fields are registered in group 0 in
//! declaration order. The compact icon payload duplicates the subset needed by
//! UI surfaces that render a small player portrait without reading every
//! detailed appearance field.

use crate::{az_rtti, replicated_state, type_registry};

use crate::Marshaler;
use crate::serialize::ReplicatedFieldHandler;

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
#[replicated_state]
#[derive(Debug, Clone, Default)]
#[az_rtti("6C597946-2506-4385-8FB4-882FB6A98D5D")]
#[type_registry(1195)]
pub struct PlayerAppearanceComponentReplicatedState {
    /// Gender selection.
    pub player_gender: ReplicatedFieldHandler<u8>,
    /// Race selection.
    pub player_race: ReplicatedFieldHandler<u8>,
    /// Skin-tone selection.
    pub player_skin_tone: ReplicatedFieldHandler<u8>,
    /// Hairstyle selection.
    pub player_hairstyle: ReplicatedFieldHandler<u8>,
    /// Facial-hair selection.
    pub player_facial_hair: ReplicatedFieldHandler<u8>,
    /// Hair-color selection.
    pub player_hair_color: ReplicatedFieldHandler<u8>,
    /// Facial-hair color selection.
    pub player_facial_hair_color: ReplicatedFieldHandler<u8>,
    /// Eye-color selection.
    pub player_eye_color: ReplicatedFieldHandler<u8>,
    /// Face-mark selection.
    pub player_face_mark: ReplicatedFieldHandler<u8>,
    /// Scar selection.
    pub player_scar: ReplicatedFieldHandler<u8>,
    /// Tattoo selection.
    pub player_tattoo: ReplicatedFieldHandler<u8>,
    /// Tattoo-color selection.
    pub player_tattoo_color: ReplicatedFieldHandler<u8>,
    /// Compact portrait/icon appearance subset.
    pub icon_n_data: ReplicatedFieldHandler<PlayerAppearanceIconData>,
    /// Version or dirty flag for appearance changes.
    pub appearance_change_flag: ReplicatedFieldHandler<u8>,
}

impl PlayerAppearanceComponentReplicatedState {
    /// Applies a full appearance snapshot to the replicated fields.
    pub fn apply_snapshot(&mut self, snapshot: PlayerAppearanceSnapshot) {
        self.player_gender.set_value(snapshot.gender);
        self.player_race.set_value(snapshot.race);
        self.player_skin_tone.set_value(snapshot.skin_tone);
        self.player_hairstyle.set_value(snapshot.hairstyle);
        self.player_facial_hair.set_value(snapshot.facial_hair);
        self.player_hair_color.set_value(snapshot.hair_color);
        self.player_facial_hair_color
            .set_value(snapshot.facial_hair_color);
        self.player_eye_color.set_value(snapshot.eye_color);
        self.player_face_mark.set_value(snapshot.face_mark);
        self.player_scar.set_value(snapshot.scar);
        self.player_tattoo.set_value(snapshot.tattoo);
        self.player_tattoo_color.set_value(snapshot.tattoo_color);
        self.icon_n_data.set_value(snapshot.icon_data());
        self.appearance_change_flag
            .set_value(snapshot.appearance_change_flag);
    }
}
