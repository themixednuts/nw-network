use crate::Marshaler;
use crate::hub::ReplicatedState;
use crate::serialize::ReplicatedFieldHandler;

/// Generated serialization shape.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Marshaler)]
pub struct PlayerAppearanceIconData {
    pub gender: u8,
    pub race: u8,
    pub skin_tone: u8,
    pub hairstyle: u8,
    pub hair_color: u8,
    pub facial_hair: u8,
    pub facial_hair_color: u8,
    pub icon_flags: u16,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlayerAppearanceSnapshot {
    pub gender: u8,
    pub race: u8,
    pub skin_tone: u8,
    pub hairstyle: u8,
    pub facial_hair: u8,
    pub hair_color: u8,
    pub facial_hair_color: u8,
    pub eye_color: u8,
    pub face_mark: u8,
    pub scar: u8,
    pub tattoo: u8,
    pub tattoo_color: u8,
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

#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::ReplicatedState,
    nw_network_derive::AzRtti,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("6C597946-2506-4385-8FB4-882FB6A98D5D")]
#[type_registry(1195)]
pub struct PlayerAppearanceComponentReplicatedState {
    pub player_gender: ReplicatedFieldHandler<u8>,
    pub player_race: ReplicatedFieldHandler<u8>,
    pub player_skin_tone: ReplicatedFieldHandler<u8>,
    pub player_hairstyle: ReplicatedFieldHandler<u8>,
    pub player_facial_hair: ReplicatedFieldHandler<u8>,
    pub player_hair_color: ReplicatedFieldHandler<u8>,
    pub player_facial_hair_color: ReplicatedFieldHandler<u8>,
    pub player_eye_color: ReplicatedFieldHandler<u8>,
    pub player_face_mark: ReplicatedFieldHandler<u8>,
    pub player_scar: ReplicatedFieldHandler<u8>,
    pub player_tattoo: ReplicatedFieldHandler<u8>,
    pub player_tattoo_color: ReplicatedFieldHandler<u8>,
    pub icon_n_data: ReplicatedFieldHandler<PlayerAppearanceIconData>,
    pub appearance_change_flag: ReplicatedFieldHandler<u8>,

    pub hub: ReplicatedState,
}

impl PlayerAppearanceComponentReplicatedState {
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
