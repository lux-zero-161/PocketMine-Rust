use serde::{Deserialize, Serialize};
use std::ops::{Add, Sub, Mul, Div};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Vector3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vector3 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0, z: 0.0 };

    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    pub fn distance(&self, other: &Vector3) -> f64 {
        self.distance_squared(other).sqrt()
    }

    pub fn distance_squared(&self, other: &Vector3) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        dx * dx + dy * dy + dz * dz
    }

    pub fn floor(&self) -> BlockPosition {
        BlockPosition {
            x: self.x.floor() as i32,
            y: self.y.floor() as i32,
            z: self.z.floor() as i32,
        }
    }
}

impl Add for Vector3 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}

impl Sub for Vector3 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

    pub fn to_vector3(&self) -> Vector3 {
        Vector3::new(self.x as f64, self.y as f64, self.z as f64)
    }

    pub fn side(&self, side: Facing) -> Self {
        match side {
            Facing::Down => Self::new(self.x, self.y - 1, self.z),
            Facing::Up => Self::new(self.x, self.y + 1, self.z),
            Facing::North => Self::new(self.x, self.y, self.z - 1),
            Facing::South => Self::new(self.x, self.y, self.z + 1),
            Facing::West => Self::new(self.x - 1, self.y, self.z),
            Facing::East => Self::new(self.x + 1, self.y, self.z),
        }
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
        Self { min_x, min_y, min_z, max_x, max_y, max_z }
    }

    pub fn intersects(&self, other: &AxisAlignedBB) -> bool {
        self.max_x > other.min_x && self.min_x < other.max_x &&
        self.max_y > other.min_y && self.min_y < other.max_y &&
        self.max_z > other.min_z && self.min_z < other.max_z
    }
}
