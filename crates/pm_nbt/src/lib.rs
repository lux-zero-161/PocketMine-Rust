//! Minimal NBT implementation for Bedrock **network** NBT.
//!
//! Bedrock's network NBT uses little-endian numbers and `u16` string lengths
//! (unlike the big-endian disk format). This is what block-state palettes,
//! player properties and chunk data need.

#[derive(Debug, Clone, PartialEq)]
pub enum Tag {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<u8>),
    String(String),
    List(Vec<Tag>),
    Compound(Vec<(String, Tag)>),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}

impl Tag {
    pub fn compound(children: Vec<(impl Into<String>, Tag)>) -> Self {
        Tag::Compound(children.into_iter().map(|(k, v)| (k.into(), v)).collect())
    }

    pub fn string(value: impl Into<String>) -> Self {
        Tag::String(value.into())
    }

    fn type_id(&self) -> u8 {
        match self {
            Tag::Byte(_) => 1,
            Tag::Short(_) => 2,
            Tag::Int(_) => 3,
            Tag::Long(_) => 4,
            Tag::Float(_) => 5,
            Tag::Double(_) => 6,
            Tag::ByteArray(_) => 7,
            Tag::String(_) => 8,
            Tag::List(_) => 9,
            Tag::Compound(_) => 10,
            Tag::IntArray(_) => 11,
            Tag::LongArray(_) => 12,
        }
    }
}

pub fn empty_compound() -> Vec<u8> {
    encode_root(&Tag::Compound(Vec::new()))
}

/// Encodes a root compound (empty name) in Bedrock network NBT.
pub fn encode_root(tag: &Tag) -> Vec<u8> {
    let Tag::Compound(children) = tag else {
        panic!("NBT root must be a compound");
    };
    let mut out = Vec::new();
    out.push(10);
    out.extend_from_slice(&0u16.to_le_bytes());
    encode_children(&mut out, children);
    out.push(0);
    out
}

fn encode_string(out: &mut Vec<u8>, value: &str) {
    out.extend_from_slice(&(value.len() as u16).to_le_bytes());
    out.extend_from_slice(value.as_bytes());
}

fn encode_children(out: &mut Vec<u8>, children: &[(String, Tag)]) {
    for (name, child) in children {
        out.push(child.type_id());
        encode_string(out, name);
        encode_payload(out, child);
    }
}

fn encode_payload(out: &mut Vec<u8>, tag: &Tag) {
    match tag {
        Tag::Byte(v) => out.push(*v as u8),
        Tag::Short(v) => out.extend_from_slice(&v.to_le_bytes()),
        Tag::Int(v) => out.extend_from_slice(&v.to_le_bytes()),
        Tag::Long(v) => out.extend_from_slice(&v.to_le_bytes()),
        Tag::Float(v) => out.extend_from_slice(&v.to_le_bytes()),
        Tag::Double(v) => out.extend_from_slice(&v.to_le_bytes()),
        Tag::ByteArray(values) => {
            out.extend_from_slice(&(values.len() as u32).to_le_bytes());
            out.extend_from_slice(values);
        }
        Tag::String(value) => encode_string(out, value),
        Tag::List(values) => {
            let element_type = values.first().map(Tag::type_id).unwrap_or(0);
            out.push(element_type);
            out.extend_from_slice(&(values.len() as u32).to_le_bytes());
            for value in values {
                encode_payload(out, value);
            }
        }
        Tag::Compound(children) => {
            encode_children(out, children);
            out.push(0);
        }
        Tag::IntArray(values) => {
            out.extend_from_slice(&(values.len() as u32).to_le_bytes());
            for value in values {
                out.extend_from_slice(&value.to_le_bytes());
            }
        }
        Tag::LongArray(values) => {
            out.extend_from_slice(&(values.len() as u32).to_le_bytes());
            for value in values {
                out.extend_from_slice(&value.to_le_bytes());
            }
        }
    }
}

/// Block-state compound used by `StartGame`'s block palette and persistent
/// chunk palettes.
pub fn block_state(name: &str, version: i32) -> Vec<u8> {
    encode_root(&Tag::compound(vec![
        ("name", Tag::string(name)),
        ("states", Tag::Compound(Vec::new())),
        ("version", Tag::Int(version)),
    ]))
}

// ---------------------------------------------------------------------------
// Reader (used by tests and future world loading)
// ---------------------------------------------------------------------------

pub fn decode_root(input: &[u8]) -> Result<Tag, String> {
    let mut offset = 0usize;
    let tag = read_tag(input, &mut offset, true)?;
    Ok(tag)
}

fn read_u16(input: &[u8], offset: &mut usize) -> Result<u16, String> {
    let bytes = input
        .get(*offset..*offset + 2)
        .ok_or("unexpected end of NBT")?;
    *offset += 2;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_u32(input: &[u8], offset: &mut usize) -> Result<u32, String> {
    let bytes = input
        .get(*offset..*offset + 4)
        .ok_or("unexpected end of NBT")?;
    *offset += 4;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn read_string(input: &[u8], offset: &mut usize) -> Result<String, String> {
    let length = read_u16(input, offset)? as usize;
    let bytes = input.get(*offset..*offset + length).ok_or_else(|| {
        format!(
            "unexpected end of NBT string at {offset} len {length} (input {})",
            input.len()
        )
    })?;
    *offset += length;
    String::from_utf8(bytes.to_vec()).map_err(|e| e.to_string())
}

fn read_tag(input: &[u8], offset: &mut usize, named: bool) -> Result<Tag, String> {
    let type_id = *input.get(*offset).ok_or("unexpected end of NBT")?;
    *offset += 1;
    if type_id == 0 {
        return Ok(Tag::Compound(Vec::new()));
    }
    if named {
        let _name = read_string(input, offset)?;
    }
    read_payload(input, offset, type_id)
}

fn read_payload(input: &[u8], offset: &mut usize, type_id: u8) -> Result<Tag, String> {
    Ok(match type_id {
        1 => {
            let value = *input.get(*offset).ok_or("unexpected end of NBT")?;
            *offset += 1;
            Tag::Byte(value as i8)
        }
        2 => {
            let value = read_u16(input, offset)?;
            Tag::Short(value as i16)
        }
        3 => {
            let value = read_u32(input, offset)?;
            Tag::Int(value as i32)
        }
        4 => {
            let bytes = input
                .get(*offset..*offset + 8)
                .ok_or("unexpected end of NBT")?;
            *offset += 8;
            Tag::Long(i64::from_le_bytes(bytes.try_into().unwrap()))
        }
        5 => {
            let value = read_u32(input, offset)?;
            Tag::Float(f32::from_bits(value))
        }
        6 => {
            let bytes = input
                .get(*offset..*offset + 8)
                .ok_or("unexpected end of NBT")?;
            *offset += 8;
            Tag::Double(f64::from_bits(u64::from_le_bytes(
                bytes.try_into().unwrap(),
            )))
        }
        7 => {
            let length = read_u32(input, offset)? as usize;
            let bytes = input
                .get(*offset..*offset + length)
                .ok_or("unexpected end of NBT byte array")?;
            *offset += length;
            Tag::ByteArray(bytes.to_vec())
        }
        8 => Tag::String(read_string(input, offset)?),
        9 => {
            let element_type = *input.get(*offset).ok_or("unexpected end of NBT")?;
            *offset += 1;
            let length = read_u32(input, offset)? as usize;
            let mut values = Vec::with_capacity(length.min(1024));
            for _ in 0..length {
                values.push(read_payload(input, offset, element_type)?);
            }
            Tag::List(values)
        }
        10 => {
            let mut children = Vec::new();
            loop {
                let child_type = *input.get(*offset).ok_or("unexpected end of NBT compound")?;
                *offset += 1;
                if child_type == 0 {
                    break;
                }
                let name = read_string_after_type(input, offset)?;
                children.push((name, read_payload(input, offset, child_type)?));
            }
            Tag::Compound(children)
        }
        11 => {
            let length = read_u32(input, offset)? as usize;
            let mut values = Vec::with_capacity(length.min(1024));
            for _ in 0..length {
                values.push(read_u32(input, offset)? as i32);
            }
            Tag::IntArray(values)
        }
        12 => {
            let length = read_u32(input, offset)? as usize;
            let mut values = Vec::with_capacity(length.min(1024));
            for _ in 0..length {
                let bytes = input
                    .get(*offset..*offset + 8)
                    .ok_or("unexpected end of NBT")?;
                *offset += 8;
                values.push(i64::from_le_bytes(bytes.try_into().unwrap()));
            }
            Tag::LongArray(values)
        }
        other => return Err(format!("unknown NBT tag {other}")),
    })
}

fn read_string_after_type(input: &[u8], offset: &mut usize) -> Result<String, String> {
    read_string(input, offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_state_roundtrip() {
        let encoded = block_state("minecraft:stone", 18168865);
        let decoded = decode_root(&encoded).unwrap();
        let Tag::Compound(children) = decoded else {
            panic!("root")
        };
        let name = children.iter().find(|(k, _)| k == "name").unwrap();
        assert_eq!(name.1, Tag::String("minecraft:stone".into()));
        let version = children.iter().find(|(k, _)| k == "version").unwrap();
        assert_eq!(version.1, Tag::Int(18168865));
        assert!(children
            .iter()
            .any(|(k, v)| k == "states" && v == &Tag::Compound(Vec::new())));
    }

    #[test]
    fn empty_compound_shape() {
        assert_eq!(empty_compound(), vec![10, 0, 0, 0]);
    }

    #[test]
    fn nested_lists_roundtrip() {
        let tag = Tag::compound(vec![
            ("ints", Tag::IntArray(vec![1, -2, 3])),
            (
                "list",
                Tag::List(vec![Tag::String("a".into()), Tag::String("b".into())]),
            ),
            ("nested", Tag::compound(vec![("x", Tag::Long(42))])),
        ]);
        let encoded = encode_root(&tag);
        assert_eq!(decode_root(&encoded).unwrap(), tag);
    }
}
