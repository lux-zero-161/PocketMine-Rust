//! Vector / block maths matching `pocketmine\math\*`.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct Vector2 {
    pub x: f64,
    pub y: f64,
}

impl Vector2 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
    pub fn length(&self) -> f64 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
    pub fn length_squared(&self) -> f64 {
        self.x * self.x + self.y * self.y
    }
    pub fn distance(&self, other: &Self) -> f64 {
        (*self - *other).length()
    }
    pub fn normalize(&self) -> Self {
        let len = self.length();
        if len > 0.0 {
            Self::new(self.x / len, self.y / len)
        } else {
            Self::ZERO
        }
    }
}

impl Add for Vector2 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y)
    }
}
impl Sub for Vector2 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.x - rhs.x, self.y - rhs.y)
    }
}
impl Mul<f64> for Vector2 {
    type Output = Self;
    fn mul(self, rhs: f64) -> Self {
        Self::new(self.x * rhs, self.y * rhs)
    }
}
impl Div<f64> for Vector2 {
    type Output = Self;
    fn div(self, rhs: f64) -> Self {
        Self::new(self.x / rhs, self.y / rhs)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct Vector3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vector3 {
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    pub const ONE: Self = Self {
        x: 1.0,
        y: 1.0,
        z: 1.0,
    };
    pub const UP: Self = Self {
        x: 0.0,
        y: 1.0,
        z: 0.0,
    };

    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    pub fn length(&self) -> f64 {
        self.length_squared().sqrt()
    }

    pub fn length_squared(&self) -> f64 {
        self.x * self.x + self.y * self.y + self.z * self.z
    }

    pub fn distance(&self, other: &Vector3) -> f64 {
        self.distance_squared(other).sqrt()
    }

    pub fn distance_squared(&self, other: &Vector3) -> f64 {
        let (dx, dy, dz) = (self.x - other.x, self.y - other.y, self.z - other.z);
        dx * dx + dy * dy + dz * dz
    }

    pub fn dot(&self, other: &Vector3) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    pub fn cross(&self, other: &Vector3) -> Vector3 {
        Vector3::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }

    pub fn normalize(&self) -> Vector3 {
        let len = self.length();
        if len > 0.0 {
            *self / len
        } else {
            Self::ZERO
        }
    }

    /// The integer block position containing this point (floor semantics).
    pub fn floor(&self) -> BlockPosition {
        BlockPosition::new(
            self.x.floor() as i32,
            self.y.floor() as i32,
            self.z.floor() as i32,
        )
    }

    /// Rounds to the nearest block position.
    pub fn round(&self) -> BlockPosition {
        BlockPosition::new(
            self.x.round() as i32,
            self.y.round() as i32,
            self.z.round() as i32,
        )
    }

    pub fn add(&self, x: f64, y: f64, z: f64) -> Self {
        Self::new(self.x + x, self.y + y, self.z + z)
    }

    pub fn subtract(&self, x: f64, y: f64, z: f64) -> Self {
        Self::new(self.x - x, self.y - y, self.z - z)
    }

    pub fn abs(&self) -> Self {
        Self::new(self.x.abs(), self.y.abs(), self.z.abs())
    }

    pub fn max(&self, other: Vector3) -> Self {
        Self::new(
            self.x.max(other.x),
            self.y.max(other.y),
            self.z.max(other.z),
        )
    }

    pub fn min(&self, other: Vector3) -> Self {
        Self::new(
            self.x.min(other.x),
            self.y.min(other.y),
            self.z.min(other.z),
        )
    }
}

impl fmt::Display for Vector3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {}, {})", self.x, self.y, self.z)
    }
}

impl Add for Vector3 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}
impl AddAssign for Vector3 {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}
impl Sub for Vector3 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}
impl SubAssign for Vector3 {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}
impl Mul<f64> for Vector3 {
    type Output = Self;
    fn mul(self, rhs: f64) -> Self::Output {
        Self::new(self.x * rhs, self.y * rhs, self.z * rhs)
    }
}
impl Div<f64> for Vector3 {
    type Output = Self;
    fn div(self, rhs: f64) -> Self::Output {
        Self::new(self.x / rhs, self.y / rhs, self.z / rhs)
    }
}
impl Neg for Vector3 {
    type Output = Self;
    fn neg(self) -> Self::Output {
        Self::new(-self.x, -self.y, -self.z)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct BlockPosition {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl BlockPosition {
    pub const ZERO: Self = Self { x: 0, y: 0, z: 0 };

    pub fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    pub fn from_vector(v: Vector3) -> Self {
        v.floor()
    }

    pub fn to_vector3(&self) -> Vector3 {
        Vector3::new(self.x as f64, self.y as f64, self.z as f64)
    }

    /// Adds `(0.5, 0.5, 0.5)` to get the centre of the block.
    pub fn center(&self) -> Vector3 {
        Vector3::new(
            self.x as f64 + 0.5,
            self.y as f64 + 0.5,
            self.z as f64 + 0.5,
        )
    }

    pub fn chunk_x(&self) -> i32 {
        self.x >> 4
    }
    pub fn chunk_z(&self) -> i32 {
        self.z >> 4
    }
    pub fn local_x(&self) -> i32 {
        self.x & 15
    }
    pub fn local_z(&self) -> i32 {
        self.z & 15
    }

    pub fn side(&self, side: Facing) -> Self {
        let (dx, dy, dz) = side.offset();
        Self::new(self.x + dx, self.y + dy, self.z + dz)
    }

    pub fn up(&self, n: i32) -> Self {
        Self::new(self.x, self.y + n, self.z)
    }
    pub fn down(&self, n: i32) -> Self {
        Self::new(self.x, self.y - n, self.z)
    }
    pub fn north(&self, n: i32) -> Self {
        Self::new(self.x, self.y, self.z - n)
    }
    pub fn south(&self, n: i32) -> Self {
        Self::new(self.x, self.y, self.z + n)
    }
    pub fn west(&self, n: i32) -> Self {
        Self::new(self.x - n, self.y, self.z)
    }
    pub fn east(&self, n: i32) -> Self {
        Self::new(self.x + n, self.y, self.z)
    }

    pub fn add(&self, x: i32, y: i32, z: i32) -> Self {
        Self::new(self.x + x, self.y + y, self.z + z)
    }

    pub fn distance_squared(&self, other: &BlockPosition) -> f64 {
        let (dx, dy, dz) = (
            (self.x - other.x) as f64,
            (self.y - other.y) as f64,
            (self.z - other.z) as f64,
        );
        dx * dx + dy * dy + dz * dz
    }

    pub fn as_vector3(&self) -> Vector3 {
        self.to_vector3()
    }
}

impl From<Vector3> for BlockPosition {
    fn from(v: Vector3) -> Self {
        v.floor()
    }
}

impl fmt::Display for BlockPosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {}, {})", self.x, self.y, self.z)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Facing {
    Down = 0,
    Up = 1,
    North = 2,
    South = 3,
    West = 4,
    East = 5,
}

impl Facing {
    pub fn offset(&self) -> (i32, i32, i32) {
        match self {
            Facing::Down => (0, -1, 0),
            Facing::Up => (0, 1, 0),
            Facing::North => (0, 0, -1),
            Facing::South => (0, 0, 1),
            Facing::West => (-1, 0, 0),
            Facing::East => (1, 0, 0),
        }
    }

    pub fn opposite(&self) -> Self {
        match self {
            Facing::Down => Facing::Up,
            Facing::Up => Facing::Down,
            Facing::North => Facing::South,
            Facing::South => Facing::North,
            Facing::West => Facing::East,
            Facing::East => Facing::West,
        }
    }

    pub fn from_offset(dx: i32, dy: i32, dz: i32) -> Option<Self> {
        match (dx.signum(), dy.signum(), dz.signum()) {
            (0, -1, 0) => Some(Facing::Down),
            (0, 1, 0) => Some(Facing::Up),
            (0, 0, -1) => Some(Facing::North),
            (0, 0, 1) => Some(Facing::South),
            (-1, 0, 0) => Some(Facing::West),
            (1, 0, 0) => Some(Facing::East),
            _ => None,
        }
    }

    pub fn axis(&self) -> Axis {
        match self {
            Facing::Down | Facing::Up => Axis::Y,
            Facing::North | Facing::South => Axis::Z,
            Facing::West | Facing::East => Axis::X,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Axis {
    X,
    Y,
    Z,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AxisAlignedBB {
    pub min_x: f64,
    pub min_y: f64,
    pub min_z: f64,
    pub max_x: f64,
    pub max_y: f64,
    pub max_z: f64,
}

impl AxisAlignedBB {
    pub fn new(min_x: f64, min_y: f64, min_z: f64, max_x: f64, max_y: f64, max_z: f64) -> Self {
        Self {
            min_x,
            min_y,
            min_z,
            max_x,
            max_y,
            max_z,
        }
    }

    /// Bounding box around a single block.
    pub fn from_block(pos: BlockPosition) -> Self {
        Self::new(
            pos.x as f64,
            pos.y as f64,
            pos.z as f64,
            pos.x as f64 + 1.0,
            pos.y as f64 + 1.0,
            pos.z as f64 + 1.0,
        )
    }

    pub fn from_center(center: Vector3, width: f64, height: f64) -> Self {
        let half = width / 2.0;
        Self::new(
            center.x - half,
            center.y,
            center.z - half,
            center.x + half,
            center.y + height,
            center.z + half,
        )
    }

    pub fn intersects(&self, other: &AxisAlignedBB) -> bool {
        self.max_x > other.min_x
            && self.min_x < other.max_x
            && self.max_y > other.min_y
            && self.min_y < other.max_y
            && self.max_z > other.min_z
            && self.min_z < other.max_z
    }

    pub fn contains(&self, point: Vector3) -> bool {
        point.x >= self.min_x
            && point.x <= self.max_x
            && point.y >= self.min_y
            && point.y <= self.max_y
            && point.z >= self.min_z
            && point.z <= self.max_z
    }

    pub fn expand(&self, x: f64, y: f64, z: f64) -> Self {
        Self::new(
            self.min_x - x,
            self.min_y - y,
            self.min_z - z,
            self.max_x + x,
            self.max_y + y,
            self.max_z + z,
        )
    }

    pub fn contract(&self, x: f64, y: f64, z: f64) -> Self {
        self.expand(-x, -y, -z)
    }

    pub fn offset(&self, x: f64, y: f64, z: f64) -> Self {
        Self::new(
            self.min_x + x,
            self.min_y + y,
            self.min_z + z,
            self.max_x + x,
            self.max_y + y,
            self.max_z + z,
        )
    }

    pub fn grow(&self, amount: f64) -> Self {
        self.expand(amount, amount, amount)
    }

    pub fn center(&self) -> Vector3 {
        Vector3::new(
            (self.min_x + self.max_x) / 2.0,
            (self.min_y + self.max_y) / 2.0,
            (self.min_z + self.max_z) / 2.0,
        )
    }
}

/// Position plus rotation, mirroring `pocketmine\world\Position`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Location {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f64,
    pub pitch: f64,
}

impl Location {
    pub fn new(x: f64, y: f64, z: f64, yaw: f64, pitch: f64) -> Self {
        Self {
            x,
            y,
            z,
            yaw,
            pitch,
        }
    }

    pub fn from_vector(v: Vector3, yaw: f64, pitch: f64) -> Self {
        Self::new(v.x, v.y, v.z, yaw, pitch)
    }

    pub fn as_vector3(&self) -> Vector3 {
        Vector3::new(self.x, self.y, self.z)
    }

    pub fn block_position(&self) -> BlockPosition {
        self.as_vector3().floor()
    }
}

/// Deterministic xorshift-based RNG used for world generation. Avoids pulling
/// in a dependency while still allowing reproducible seeds.
#[derive(Debug, Clone)]
pub struct Random {
    state: u64,
}

impl Random {
    pub fn new(seed: u64) -> Self {
        Self {
            state: seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407)
                | 1,
        }
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    pub fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }

    pub fn next_i32(&mut self, bound: i32) -> i32 {
        if bound <= 0 {
            return 0;
        }
        (self.next_u64() % bound as u64) as i32
    }

    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn chance(&mut self, probability: f64) -> bool {
        self.next_f64() < probability
    }
}
