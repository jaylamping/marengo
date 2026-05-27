/// Unit quaternion from BNO085 rotation vector (i, j, k, real).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quaternion {
    pub i: f64,
    pub j: f64,
    pub k: f64,
    pub real: f64,
}

impl Quaternion {
    pub fn normalize(mut self) -> Self {
        let norm =
            (self.i * self.i + self.j * self.j + self.k * self.k + self.real * self.real).sqrt();
        if norm > f64::EPSILON {
            self.i /= norm;
            self.j /= norm;
            self.k /= norm;
            self.real /= norm;
        }
        self
    }
}

/// SH-2 accuracy nibble (0 = unreliable .. 3 = high).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImuAccuracy {
    Unreliable,
    Low,
    Medium,
    High,
}

impl From<u8> for ImuAccuracy {
    fn from(value: u8) -> Self {
        match value & 0b11 {
            1 => Self::Low,
            2 => Self::Medium,
            3 => Self::High,
            _ => Self::Unreliable,
        }
    }
}

/// Parsed rotation-vector report.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RotationVectorSample {
    pub quaternion: Quaternion,
    pub accuracy: ImuAccuracy,
}

/// Typed IMU sample for future Chappe/proto wiring.
#[derive(Debug, Clone, PartialEq)]
pub struct ImuSample {
    pub frame_id: String,
    pub rotation: RotationVectorSample,
    pub accelerometer_m_s2: Option<[f64; 3]>,
    pub gyroscope_rad_s: Option<[f64; 3]>,
}
