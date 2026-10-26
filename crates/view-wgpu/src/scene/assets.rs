//! Small, bounded scene asset and transform-animation services.
use super::{Mesh3D, RenderError, Transform3D};
use std::{error::Error, fmt};

/// Resource caps applied before an OBJ mesh is constructed.
#[derive(Clone, Copy, Debug)]
pub struct ObjLimits {
    pub max_bytes: usize,
    pub max_positions: usize,
    pub max_faces: usize,
    pub max_triangles: usize,
}

impl Default for ObjLimits {
    fn default() -> Self {
        Self {
            max_bytes: 16 * 1024 * 1024,
            max_positions: 1_000_000,
            max_faces: 1_000_000,
            max_triangles: 2_000_000,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ObjError {
    LimitExceeded(&'static str),
    InvalidRecord { line: usize, record: &'static str },
    InvalidIndex { line: usize },
    NonFinitePosition { line: usize },
    Unsupported { line: usize, feature: &'static str },
    InvalidMesh,
}

impl fmt::Display for ObjError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LimitExceeded(name) => write!(f, "OBJ limit exceeded: {name}"),
            Self::InvalidRecord { line, record } => {
                write!(f, "invalid OBJ {record} record on line {line}")
            }
            Self::InvalidIndex { line } => write!(f, "invalid OBJ position index on line {line}"),
            Self::NonFinitePosition { line } => write!(f, "non-finite OBJ position on line {line}"),
            Self::Unsupported { line, feature } => {
                write!(f, "unsupported OBJ {feature} on line {line}")
            }
            Self::InvalidMesh => f.write_str("OBJ did not contain any triangles"),
        }
    }
}
impl Error for ObjError {}

/// Import the positions-only OBJ profile into an indexed triangle mesh.
/// Supports `v` and `f` records (including relative negative position indices),
/// plus ignored object/group/smoothing labels. Polygons are triangulated as a
/// fan. Materials, normals, texture coordinates, and unknown records reject.
pub fn import_obj_positions(source: &str, limits: ObjLimits) -> Result<Mesh3D, ObjError> {
    if source.len() > limits.max_bytes {
        return Err(ObjError::LimitExceeded("bytes"));
    }
    let mut positions = Vec::new();
    let mut indices = Vec::new();
    let mut faces = 0usize;
    for (line_index, raw) in source.lines().enumerate() {
        let line = line_index + 1;
        let content = raw.split_once('#').map_or(raw, |(before, _)| before).trim();
        if content.is_empty() {
            continue;
        }
        let mut words = content.split_whitespace();
        let record = words.next().expect("nonempty record");
        match record {
            "v" => {
                let coords: Vec<_> = words.collect();
                if coords.len() != 3 {
                    return Err(ObjError::InvalidRecord {
                        line,
                        record: "vertex",
                    });
                }
                if positions.len() >= limits.max_positions {
                    return Err(ObjError::LimitExceeded("positions"));
                }
                let mut point = [0.0; 3];
                for (i, token) in coords.into_iter().enumerate() {
                    let value: f32 = token.parse().map_err(|_| ObjError::InvalidRecord {
                        line,
                        record: "vertex",
                    })?;
                    if !value.is_finite() {
                        return Err(ObjError::NonFinitePosition { line });
                    }
                    point[i] = value;
                }
                positions.push(point);
            }
            "f" => {
                faces += 1;
                if faces > limits.max_faces {
                    return Err(ObjError::LimitExceeded("faces"));
                }
                let tokens: Vec<_> = words.collect();
                if tokens.len() < 3 {
                    return Err(ObjError::InvalidRecord {
                        line,
                        record: "face",
                    });
                }
                if indices
                    .len()
                    .saturating_add((tokens.len() - 2).saturating_mul(3))
                    > limits.max_triangles.saturating_mul(3)
                {
                    return Err(ObjError::LimitExceeded("triangles"));
                }
                let mut face = Vec::with_capacity(tokens.len());
                for token in tokens {
                    if token.contains('/') {
                        let feature = if token.split('/').count() >= 3 && !token.ends_with('/') {
                            "normals/texture coordinates"
                        } else {
                            "texture coordinates"
                        };
                        return Err(ObjError::Unsupported { line, feature });
                    }
                    let raw_index: i64 =
                        token.parse().map_err(|_| ObjError::InvalidIndex { line })?;
                    let resolved = if raw_index > 0 {
                        raw_index - 1
                    } else if raw_index < 0 {
                        positions.len() as i64 + raw_index
                    } else {
                        -1
                    };
                    if resolved < 0
                        || resolved as usize >= positions.len()
                        || resolved > i64::from(u32::MAX)
                    {
                        return Err(ObjError::InvalidIndex { line });
                    }
                    face.push(resolved as u32);
                }
                for corner in 1..face.len() - 1 {
                    indices.extend_from_slice(&[face[0], face[corner], face[corner + 1]]);
                }
            }
            "o" | "g" | "s" => {}
            "vn" => {
                return Err(ObjError::Unsupported {
                    line,
                    feature: "normals",
                });
            }
            "vt" => {
                return Err(ObjError::Unsupported {
                    line,
                    feature: "texture coordinates",
                });
            }
            "mtllib" | "usemtl" | "newmtl" => {
                return Err(ObjError::Unsupported {
                    line,
                    feature: "materials",
                });
            }
            _ => {
                return Err(ObjError::Unsupported {
                    line,
                    feature: "record",
                });
            }
        }
    }
    if indices.is_empty() {
        return Err(ObjError::InvalidMesh);
    }
    Mesh3D::new(positions, indices).map_err(|_: RenderError| ObjError::InvalidMesh)
}

/// Position at one point in a translation animation track.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TranslationKeyframe {
    pub time_seconds: f64,
    pub translation: [f32; 3],
}

/// Validated, linearly interpolated world/local translation track.
#[derive(Clone, Debug, PartialEq)]
pub struct TranslationAnimation {
    keyframes: Vec<TranslationKeyframe>,
}

impl TranslationAnimation {
    pub const MAX_KEYFRAMES: usize = 100_000;

    pub fn new(keyframes: Vec<TranslationKeyframe>) -> Result<Self, RenderError> {
        if keyframes.is_empty() || keyframes.len() > Self::MAX_KEYFRAMES {
            return Err(RenderError::Invalid(
                "invalid animation keyframe count".into(),
            ));
        }
        let mut previous = None;
        for key in &keyframes {
            if !key.time_seconds.is_finite()
                || key.time_seconds < 0.0
                || !key.translation.iter().all(|v| v.is_finite())
            {
                return Err(RenderError::Invalid(
                    "non-finite or negative animation keyframe".into(),
                ));
            }
            if previous.is_some_and(|time| key.time_seconds <= time) {
                return Err(RenderError::Invalid(
                    "animation times must strictly increase".into(),
                ));
            }
            previous = Some(key.time_seconds);
        }
        Ok(Self { keyframes })
    }

    pub fn keyframes(&self) -> &[TranslationKeyframe] {
        &self.keyframes
    }

    /// Clamp outside the track and linearly interpolate between adjacent keys.
    pub fn sample(&self, time_seconds: f64) -> Result<[f32; 3], RenderError> {
        if !time_seconds.is_finite() || time_seconds < 0.0 {
            return Err(RenderError::Invalid("invalid animation sample time".into()));
        }
        if time_seconds <= self.keyframes[0].time_seconds {
            return Ok(self.keyframes[0].translation);
        }
        let last = *self.keyframes.last().expect("validated nonempty track");
        if time_seconds >= last.time_seconds {
            return Ok(last.translation);
        }
        let upper = self
            .keyframes
            .partition_point(|key| key.time_seconds < time_seconds);
        let a = self.keyframes[upper - 1];
        let b = self.keyframes[upper];
        let fraction = ((time_seconds - a.time_seconds) / (b.time_seconds - a.time_seconds)) as f32;
        let translation = std::array::from_fn(|i| {
            a.translation[i] + (b.translation[i] - a.translation[i]) * fraction
        });
        if translation.iter().all(|v| v.is_finite()) {
            Ok(translation)
        } else {
            Err(RenderError::Invalid(
                "animation interpolation overflow".into(),
            ))
        }
    }

    /// Produce a pure translation transform for callers that need a standalone
    /// transform instead of applying the sample to a scene object's transform.
    pub fn transform_at(&self, time_seconds: f64) -> Result<Transform3D, RenderError> {
        Transform3D::translation(self.sample(time_seconds)?)
    }
}
