use arrayvec::ArrayVec;
use bevy_math::Vec3;

use crate::hub::ReplicatedState;
use crate::serialize::{
    HalfF32, Marshaler, MarshalerError, ReadBuffer, ReplicatedFieldHandler, VlqU32Marshaler,
    WriteBuffer,
};

pub const MAX_CLIENT_PATHING_CORRIDOR_PATHS: usize = 6;
pub const MAX_CLIENT_PATHING_CORRIDOR_POINTS: usize = 49;
pub const MAX_CLIENT_PATHING_CORRIDOR_SAMPLES: usize = 50;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClientPathingCorridorPath {
    pub start: Vec3,
    pub width: f32,
    pub points: ArrayVec<Vec3, MAX_CLIENT_PATHING_CORRIDOR_POINTS>,
    pub samples: ArrayVec<f32, MAX_CLIENT_PATHING_CORRIDOR_SAMPLES>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClientPathingCorridorPaths {
    pub paths: ArrayVec<Option<ClientPathingCorridorPath>, MAX_CLIENT_PATHING_CORRIDOR_PATHS>,
    pub trailing_value: f32,
}

impl Default for ClientPathingCorridorPaths {
    fn default() -> Self {
        Self {
            paths: ArrayVec::new(),
            trailing_value: -1.0,
        }
    }
}

impl Marshaler for ClientPathingCorridorPaths {
    fn marshal(&self, wb: &mut WriteBuffer) {
        VlqU32Marshaler.marshal(
            wb,
            u32::try_from(self.paths.len()).expect("corridor path cap fits in u32"),
        );
        for path in &self.paths {
            path.is_some().marshal(wb);
            if let Some(path) = path {
                marshal_corridor_path(path, wb);
            }
        }
        self.trailing_value.marshal(wb);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let count = usize::try_from(VlqU32Marshaler.unmarshal(rb)?).map_err(|_| {
            MarshalerError::ContainerOverflow {
                len: usize::MAX,
                capacity: MAX_CLIENT_PATHING_CORRIDOR_PATHS,
            }
        })?;
        check_count(count, MAX_CLIENT_PATHING_CORRIDOR_PATHS)?;

        let mut paths = ArrayVec::new();
        for _ in 0..count {
            let has_path = bool::unmarshal(rb)?;
            paths.push(if has_path {
                Some(unmarshal_corridor_path(rb)?)
            } else {
                None
            });
        }

        Ok(Self {
            paths,
            trailing_value: f32::unmarshal(rb)?,
        })
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
#[az_rtti("A32A95F6-9EF4-4139-8FC3-98C712910DAD")]
#[type_registry(5915)]
pub struct ClientPathingComponentReplicatedState {
    pub corridor_paths: ReplicatedFieldHandler<ClientPathingCorridorPaths>,

    pub hub: ReplicatedState,
}

fn marshal_corridor_path(path: &ClientPathingCorridorPath, wb: &mut WriteBuffer) {
    path.start.marshal(wb);
    path.width.marshal(wb);

    VlqU32Marshaler.marshal(
        wb,
        u32::try_from(path.points.len()).expect("corridor point cap fits in u32"),
    );
    for point in &path.points {
        marshal_half_vec3(*point, wb);
    }

    VlqU32Marshaler.marshal(
        wb,
        u32::try_from(path.samples.len()).expect("corridor sample cap fits in u32"),
    );
    for sample in &path.samples {
        HalfF32(*sample).marshal(wb);
    }
}

fn unmarshal_corridor_path(
    rb: &mut ReadBuffer,
) -> Result<ClientPathingCorridorPath, MarshalerError> {
    let start = Vec3::unmarshal(rb)?;
    let width = f32::unmarshal(rb)?;

    let point_count = usize::try_from(VlqU32Marshaler.unmarshal(rb)?).map_err(|_| {
        MarshalerError::ContainerOverflow {
            len: usize::MAX,
            capacity: MAX_CLIENT_PATHING_CORRIDOR_POINTS,
        }
    })?;
    check_count(point_count, MAX_CLIENT_PATHING_CORRIDOR_POINTS)?;
    let mut points = ArrayVec::new();
    for _ in 0..point_count {
        points.push(unmarshal_half_vec3(rb)?);
    }

    let sample_count = usize::try_from(VlqU32Marshaler.unmarshal(rb)?).map_err(|_| {
        MarshalerError::ContainerOverflow {
            len: usize::MAX,
            capacity: MAX_CLIENT_PATHING_CORRIDOR_SAMPLES,
        }
    })?;
    check_count(sample_count, MAX_CLIENT_PATHING_CORRIDOR_SAMPLES)?;
    let mut samples = ArrayVec::new();
    for _ in 0..sample_count {
        let HalfF32(sample) = HalfF32::unmarshal(rb)?;
        samples.push(sample);
    }

    Ok(ClientPathingCorridorPath {
        start,
        width,
        points,
        samples,
    })
}

fn marshal_half_vec3(value: Vec3, wb: &mut WriteBuffer) {
    HalfF32(value.x).marshal(wb);
    HalfF32(value.y).marshal(wb);
    HalfF32(value.z).marshal(wb);
}

fn unmarshal_half_vec3(rb: &mut ReadBuffer) -> Result<Vec3, MarshalerError> {
    let HalfF32(x) = HalfF32::unmarshal(rb)?;
    let HalfF32(y) = HalfF32::unmarshal(rb)?;
    let HalfF32(z) = HalfF32::unmarshal(rb)?;
    Ok(Vec3::new(x, y, z))
}

fn check_count(count: usize, capacity: usize) -> Result<(), MarshalerError> {
    if count > capacity {
        return Err(MarshalerError::ContainerOverflow {
            len: count,
            capacity,
        });
    }
    Ok(())
}
