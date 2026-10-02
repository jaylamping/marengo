//! Private versioned exact-value disk codec. Parsed data cannot grant reference.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;

use serde::de::{self, IntoDeserializer, Visitor};
use serde::ser::{self, SerializeMap, SerializeSeq, SerializeStruct};
use serde::Serialize;

pub(super) const BODY_CAPACITY: usize = 1024 * 1024;
const NODE_CAPACITY: usize = 65_536;
const COLLECTION_CAPACITY: usize = 8192;
const DEPTH_CAPACITY: usize = 64;
const MAGIC: &[u8; 8] = b"MRREF\0\x01\0";

#[derive(Debug, thiserror::Error)]
#[error("reference codec: {0}")]
pub(super) struct CodecError(String);

impl ser::Error for CodecError {
    fn custom<T: fmt::Display>(message: T) -> Self {
        Self(crate::bounded_message(&message.to_string()))
    }
}
impl de::Error for CodecError {
    fn custom<T: fmt::Display>(message: T) -> Self {
        Self(crate::bounded_message(&message.to_string()))
    }
}

fn invalid(message: &str) -> CodecError {
    CodecError(message.into())
}

/// Scalar tags preserve widths; floats contain their original IEEE bits.
/// Maps are canonical string-key field maps, with no JSON numeric conversion.
#[derive(Debug, PartialEq)]
enum Value {
    Bool(bool),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    U128(u128),
    I128(i128),
    F32(u32),
    F64(u64),
    String(String),
    Bytes(Vec<u8>),
    Seq(Vec<Value>),
    Map(BTreeMap<String, Value>),
    None,
    Some(Box<Value>),
    Unit,
}

#[derive(Default)]
struct Budget {
    bytes: Cell<usize>,
    nodes: Cell<usize>,
}

impl Budget {
    fn reserve(&self, bytes: usize) -> Result<(), CodecError> {
        let next = self
            .bytes
            .get()
            .checked_add(bytes)
            .filter(|next| *next <= BODY_CAPACITY - MAGIC.len())
            .ok_or_else(|| invalid("encoded body exceeds 1 MiB"))?;
        let nodes = self
            .nodes
            .get()
            .checked_add(1)
            .filter(|next| *next <= NODE_CAPACITY)
            .ok_or_else(|| invalid("too many encoded fields"))?;
        self.bytes.set(next);
        self.nodes.set(nodes);
        Ok(())
    }
}

#[derive(Clone, Default)]
struct Encoder {
    budget: Rc<Budget>,
    depth: usize,
}

impl Encoder {
    fn child(&self) -> Result<Self, CodecError> {
        let depth = self
            .depth
            .checked_add(1)
            .filter(|n| *n <= DEPTH_CAPACITY)
            .ok_or_else(|| invalid("field depth exceeds capacity"))?;
        Ok(Self {
            budget: Rc::clone(&self.budget),
            depth,
        })
    }

    fn string(&self, text: &str) -> Result<Value, CodecError> {
        self.budget.reserve(
            5usize
                .checked_add(text.len())
                .ok_or_else(|| invalid("string length overflow"))?,
        )?;
        Ok(Value::String(text.to_owned()))
    }

    fn sequence(self, length: Option<usize>) -> Result<Sequence, CodecError> {
        if length.is_some_and(|n| n > COLLECTION_CAPACITY) {
            return Err(invalid("collection exceeds capacity"));
        }
        self.budget.reserve(5)?;
        Ok(Sequence {
            encoder: self.child()?,
            values: Vec::new(),
        })
    }

    fn fields(self, length: Option<usize>) -> Result<Fields, CodecError> {
        if length.is_some_and(|n| n > COLLECTION_CAPACITY) {
            return Err(invalid("field map exceeds capacity"));
        }
        self.budget.reserve(5)?;
        Ok(Fields {
            encoder: self.child()?,
            values: BTreeMap::new(),
            key: None,
        })
    }
}

macro_rules! encode_integer {
    ($method:ident, $ty:ty, $variant:ident) => {
        fn $method(self, value: $ty) -> Result<Value, CodecError> {
            self.budget.reserve(1 + std::mem::size_of::<$ty>())?;
            Ok(Value::$variant(value))
        }
    };
}

impl ser::Serializer for Encoder {
    type Ok = Value;
    type Error = CodecError;
    type SerializeSeq = Sequence;
    type SerializeTuple = Sequence;
    type SerializeTupleStruct = Sequence;
    type SerializeTupleVariant = ser::Impossible<Value, CodecError>;
    type SerializeMap = Fields;
    type SerializeStruct = Fields;
    type SerializeStructVariant = ser::Impossible<Value, CodecError>;

    fn serialize_bool(self, value: bool) -> Result<Value, CodecError> {
        self.budget.reserve(1)?;
        Ok(Value::Bool(value))
    }
    encode_integer!(serialize_u8, u8, U8);
    encode_integer!(serialize_u16, u16, U16);
    encode_integer!(serialize_u32, u32, U32);
    encode_integer!(serialize_u64, u64, U64);
    encode_integer!(serialize_u128, u128, U128);
    encode_integer!(serialize_i8, i8, I8);
    encode_integer!(serialize_i16, i16, I16);
    encode_integer!(serialize_i32, i32, I32);
    encode_integer!(serialize_i64, i64, I64);
    encode_integer!(serialize_i128, i128, I128);

    fn serialize_f32(self, value: f32) -> Result<Value, CodecError> {
        if !value.is_finite() {
            return Err(invalid("nonfinite f32"));
        }
        self.budget.reserve(5)?;
        Ok(Value::F32(value.to_bits()))
    }
    fn serialize_f64(self, value: f64) -> Result<Value, CodecError> {
        if !value.is_finite() {
            return Err(invalid("nonfinite f64"));
        }
        self.budget.reserve(9)?;
        Ok(Value::F64(value.to_bits()))
    }
    fn serialize_char(self, _: char) -> Result<Value, CodecError> {
        Err(invalid("unsupported character field"))
    }
    fn serialize_str(self, value: &str) -> Result<Value, CodecError> {
        self.string(value)
    }
    fn serialize_bytes(self, value: &[u8]) -> Result<Value, CodecError> {
        self.budget.reserve(
            5usize
                .checked_add(value.len())
                .ok_or_else(|| invalid("byte length overflow"))?,
        )?;
        Ok(Value::Bytes(value.to_vec()))
    }
    fn serialize_none(self) -> Result<Value, CodecError> {
        self.budget.reserve(1)?;
        Ok(Value::None)
    }
    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<Value, CodecError> {
        self.budget.reserve(1)?;
        Ok(Value::Some(Box::new(value.serialize(self.child()?)?)))
    }
    fn serialize_unit(self) -> Result<Value, CodecError> {
        self.budget.reserve(1)?;
        Ok(Value::Unit)
    }
    fn serialize_unit_struct(self, _: &'static str) -> Result<Value, CodecError> {
        self.serialize_unit()
    }
    fn serialize_unit_variant(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
    ) -> Result<Value, CodecError> {
        self.string(variant)
    }
    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _: &'static str,
        value: &T,
    ) -> Result<Value, CodecError> {
        value.serialize(self.child()?)
    }
    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: &T,
    ) -> Result<Value, CodecError> {
        Err(invalid("unsupported newtype variant"))
    }
    fn serialize_seq(self, length: Option<usize>) -> Result<Sequence, CodecError> {
        self.sequence(length)
    }
    fn serialize_tuple(self, length: usize) -> Result<Sequence, CodecError> {
        self.sequence(Some(length))
    }
    fn serialize_tuple_struct(
        self,
        _: &'static str,
        length: usize,
    ) -> Result<Sequence, CodecError> {
        self.sequence(Some(length))
    }
    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleVariant, CodecError> {
        Err(invalid("unsupported tuple variant"))
    }
    fn serialize_map(self, length: Option<usize>) -> Result<Fields, CodecError> {
        self.fields(length)
    }
    fn serialize_struct(self, _: &'static str, length: usize) -> Result<Fields, CodecError> {
        self.fields(Some(length))
    }
    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeStructVariant, CodecError> {
        Err(invalid("unsupported struct variant"))
    }
    fn is_human_readable(&self) -> bool {
        false
    }
}

struct Sequence {
    encoder: Encoder,
    values: Vec<Value>,
}
impl SerializeSeq for Sequence {
    type Ok = Value;
    type Error = CodecError;
    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), CodecError> {
        if self.values.len() == COLLECTION_CAPACITY {
            return Err(invalid("collection capacity"));
        }
        self.values.push(value.serialize(self.encoder.clone())?);
        Ok(())
    }
    fn end(self) -> Result<Value, CodecError> {
        Ok(Value::Seq(self.values))
    }
}
impl ser::SerializeTuple for Sequence {
    type Ok = Value;
    type Error = CodecError;
    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), CodecError> {
        SerializeSeq::serialize_element(self, value)
    }
    fn end(self) -> Result<Value, CodecError> {
        SerializeSeq::end(self)
    }
}
impl ser::SerializeTupleStruct for Sequence {
    type Ok = Value;
    type Error = CodecError;
    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), CodecError> {
        SerializeSeq::serialize_element(self, value)
    }
    fn end(self) -> Result<Value, CodecError> {
        SerializeSeq::end(self)
    }
}

struct Fields {
    encoder: Encoder,
    values: BTreeMap<String, Value>,
    key: Option<String>,
}
impl Fields {
    fn insert<T: ?Sized + Serialize>(&mut self, key: String, value: &T) -> Result<(), CodecError> {
        if self.values.len() == COLLECTION_CAPACITY || self.values.contains_key(&key) {
            return Err(invalid("duplicate field or field capacity"));
        }
        self.values
            .insert(key, value.serialize(self.encoder.clone())?);
        Ok(())
    }
}
impl SerializeMap for Fields {
    type Ok = Value;
    type Error = CodecError;
    fn serialize_key<T: ?Sized + Serialize>(&mut self, key: &T) -> Result<(), CodecError> {
        if self.key.is_some() {
            return Err(invalid("map key without value"));
        }
        let Value::String(key) = key.serialize(self.encoder.clone())? else {
            return Err(invalid("map keys must be strings"));
        };
        self.key = Some(key);
        Ok(())
    }
    fn serialize_value<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), CodecError> {
        let key = self
            .key
            .take()
            .ok_or_else(|| invalid("map value without key"))?;
        self.insert(key, value)
    }
    fn end(self) -> Result<Value, CodecError> {
        if self.key.is_some() {
            return Err(invalid("unfinished map key"));
        }
        Ok(Value::Map(self.values))
    }
}
impl SerializeStruct for Fields {
    type Ok = Value;
    type Error = CodecError;
    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), CodecError> {
        let Value::String(key) = self.encoder.string(key)? else {
            return Err(invalid("field key encoding"));
        };
        self.insert(key, value)
    }
    fn end(self) -> Result<Value, CodecError> {
        Ok(Value::Map(self.values))
    }
}

pub(super) fn encode<T: ?Sized + Serialize>(value: &T) -> Result<Vec<u8>, CodecError> {
    let encoder = Encoder::default();
    let tree = value.serialize(encoder.clone())?;
    let mut bytes = Vec::with_capacity(MAGIC.len() + encoder.budget.bytes.get());
    bytes.extend_from_slice(MAGIC);
    write_value(&tree, &mut bytes)?;
    if bytes.len() != MAGIC.len() + encoder.budget.bytes.get() || bytes.len() > BODY_CAPACITY {
        return Err(invalid("encoding size mismatch"));
    }
    Ok(bytes)
}

fn length(length: usize, bytes: &mut Vec<u8>) -> Result<(), CodecError> {
    bytes.extend_from_slice(
        &u32::try_from(length)
            .map_err(|_| invalid("length overflow"))?
            .to_le_bytes(),
    );
    Ok(())
}

macro_rules! write_number {
    ($bytes:expr, $tag:expr, $number:expr) => {{
        $bytes.push($tag);
        $bytes.extend_from_slice(&$number.to_le_bytes());
    }};
}
fn write_value(value: &Value, bytes: &mut Vec<u8>) -> Result<(), CodecError> {
    match value {
        Value::Bool(value) => bytes.push(u8::from(*value)),
        Value::U8(n) => write_number!(bytes, 2, n),
        Value::U16(n) => write_number!(bytes, 3, n),
        Value::U32(n) => write_number!(bytes, 4, n),
        Value::U64(n) => write_number!(bytes, 5, n),
        Value::I8(n) => write_number!(bytes, 6, n),
        Value::I16(n) => write_number!(bytes, 7, n),
        Value::I32(n) => write_number!(bytes, 8, n),
        Value::I64(n) => write_number!(bytes, 9, n),
        Value::U128(n) => write_number!(bytes, 10, n),
        Value::F32(n) => write_number!(bytes, 11, n),
        Value::F64(n) => write_number!(bytes, 12, n),
        Value::I128(n) => write_number!(bytes, 13, n),
        Value::String(text) => {
            bytes.push(14);
            length(text.len(), bytes)?;
            bytes.extend_from_slice(text.as_bytes());
        }
        Value::Bytes(data) => {
            bytes.push(15);
            length(data.len(), bytes)?;
            bytes.extend_from_slice(data);
        }
        Value::Seq(values) => {
            bytes.push(16);
            length(values.len(), bytes)?;
            for value in values {
                write_value(value, bytes)?;
            }
        }
        Value::Map(fields) => {
            bytes.push(17);
            length(fields.len(), bytes)?;
            for (key, value) in fields {
                bytes.push(14);
                length(key.len(), bytes)?;
                bytes.extend_from_slice(key.as_bytes());
                write_value(value, bytes)?;
            }
        }
        Value::None => bytes.push(18),
        Value::Some(value) => {
            bytes.push(19);
            write_value(value, bytes)?;
        }
        Value::Unit => bytes.push(20),
    }
    Ok(())
}

struct Reader<'a> {
    bytes: &'a [u8],
    cursor: usize,
    nodes: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], CodecError> {
        let end = self
            .cursor
            .checked_add(count)
            .filter(|end| *end <= self.bytes.len())
            .ok_or_else(|| invalid("truncated field"))?;
        let bytes = &self.bytes[self.cursor..end];
        self.cursor = end;
        Ok(bytes)
    }
    fn number<const N: usize>(&mut self) -> Result<[u8; N], CodecError> {
        self.take(N)?
            .try_into()
            .map_err(|_| invalid("numeric width"))
    }
    fn length(&mut self) -> Result<usize, CodecError> {
        usize::try_from(u32::from_le_bytes(self.number()?)).map_err(|_| invalid("length overflow"))
    }
    fn value(&mut self, depth: usize) -> Result<Value, CodecError> {
        self.nodes = self
            .nodes
            .checked_add(1)
            .ok_or_else(|| invalid("node count"))?;
        if self.nodes > NODE_CAPACITY || depth > DEPTH_CAPACITY {
            return Err(invalid("decode capacity"));
        }
        let tag = self.number::<1>()?[0];
        Ok(match tag {
            0 => Value::Bool(false),
            1 => Value::Bool(true),
            2 => Value::U8(u8::from_le_bytes(self.number()?)),
            3 => Value::U16(u16::from_le_bytes(self.number()?)),
            4 => Value::U32(u32::from_le_bytes(self.number()?)),
            5 => Value::U64(u64::from_le_bytes(self.number()?)),
            6 => Value::I8(i8::from_le_bytes(self.number()?)),
            7 => Value::I16(i16::from_le_bytes(self.number()?)),
            8 => Value::I32(i32::from_le_bytes(self.number()?)),
            9 => Value::I64(i64::from_le_bytes(self.number()?)),
            10 => Value::U128(u128::from_le_bytes(self.number()?)),
            11 => {
                let bits = u32::from_le_bytes(self.number()?);
                if !f32::from_bits(bits).is_finite() {
                    return Err(invalid("nonfinite f32"));
                }
                Value::F32(bits)
            }
            12 => {
                let bits = u64::from_le_bytes(self.number()?);
                if !f64::from_bits(bits).is_finite() {
                    return Err(invalid("nonfinite f64"));
                }
                Value::F64(bits)
            }
            13 => Value::I128(i128::from_le_bytes(self.number()?)),
            14 => {
                let length = self.length()?;
                let text = std::str::from_utf8(self.take(length)?)
                    .map_err(|_| invalid("invalid UTF-8"))?;
                Value::String(text.to_owned())
            }
            15 => {
                let length = self.length()?;
                Value::Bytes(self.take(length)?.to_vec())
            }
            16 => {
                let count = self.length()?;
                if count > COLLECTION_CAPACITY {
                    return Err(invalid("collection capacity"));
                }
                let mut values = Vec::new();
                for _ in 0..count {
                    values.push(self.value(depth + 1)?);
                }
                Value::Seq(values)
            }
            17 => {
                let count = self.length()?;
                if count > COLLECTION_CAPACITY {
                    return Err(invalid("field capacity"));
                }
                let mut fields = BTreeMap::new();
                let mut previous = None;
                for _ in 0..count {
                    let Value::String(key) = self.value(depth + 1)? else {
                        return Err(invalid("nonstring field key"));
                    };
                    if previous.as_ref().is_some_and(|old: &String| old >= &key) {
                        return Err(invalid("duplicate or unordered fields"));
                    }
                    previous = Some(key.clone());
                    fields.insert(key, self.value(depth + 1)?);
                }
                Value::Map(fields)
            }
            18 => Value::None,
            19 => Value::Some(Box::new(self.value(depth + 1)?)),
            20 => Value::Unit,
            _ => return Err(invalid("unsupported tag")),
        })
    }
}

#[derive(Clone, Copy)]
struct Decoder<'a>(&'a Value);
impl<'de> IntoDeserializer<'de, CodecError> for Decoder<'de> {
    type Deserializer = Self;
    fn into_deserializer(self) -> Self {
        self
    }
}
impl<'de> de::Deserializer<'de> for Decoder<'de> {
    type Error = CodecError;
    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, CodecError> {
        match self.0 {
            Value::Bool(n) => visitor.visit_bool(*n),
            Value::U8(n) => visitor.visit_u8(*n),
            Value::U16(n) => visitor.visit_u16(*n),
            Value::U32(n) => visitor.visit_u32(*n),
            Value::U64(n) => visitor.visit_u64(*n),
            Value::I8(n) => visitor.visit_i8(*n),
            Value::I16(n) => visitor.visit_i16(*n),
            Value::I32(n) => visitor.visit_i32(*n),
            Value::I64(n) => visitor.visit_i64(*n),
            Value::U128(n) => visitor.visit_u128(*n),
            Value::I128(n) => visitor.visit_i128(*n),
            Value::F32(n) => visitor.visit_f32(f32::from_bits(*n)),
            Value::F64(n) => visitor.visit_f64(f64::from_bits(*n)),
            Value::String(text) => visitor.visit_borrowed_str(text),
            Value::Bytes(bytes) => visitor.visit_borrowed_bytes(bytes),
            Value::None => visitor.visit_none(),
            Value::Some(value) => visitor.visit_some(Decoder(value)),
            Value::Unit => visitor.visit_unit(),
            Value::Seq(values) => {
                visitor.visit_seq(de::value::SeqDeserializer::new(values.iter().map(Decoder)))
            }
            Value::Map(fields) => visitor.visit_map(de::value::MapDeserializer::new(
                fields.iter().map(|(key, value)| {
                    (
                        de::value::BorrowedStrDeserializer::<CodecError>::new(key),
                        Decoder(value),
                    )
                }),
            )),
        }
    }
    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, CodecError> {
        match self.0 {
            Value::None => visitor.visit_none(),
            Value::Some(value) => visitor.visit_some(Decoder(value)),
            _ => Err(invalid("option tag required")),
        }
    }
    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _: &'static str,
        _: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, CodecError> {
        let Value::String(variant) = self.0 else {
            return Err(invalid("unit enum tag required"));
        };
        visitor.visit_enum(de::value::BorrowedStrDeserializer::<CodecError>::new(
            variant,
        ))
    }
    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string bytes byte_buf
        unit unit_struct newtype_struct seq tuple tuple_struct map struct identifier ignored_any
    }
    fn is_human_readable(&self) -> bool {
        false
    }
}

pub(super) fn decode<T: de::DeserializeOwned + Serialize>(bytes: &[u8]) -> Result<T, CodecError> {
    if bytes.len() > BODY_CAPACITY || !bytes.starts_with(MAGIC) {
        return Err(invalid("body capacity or version"));
    }
    let mut reader = Reader {
        bytes,
        cursor: MAGIC.len(),
        nodes: 0,
    };
    let value = reader.value(0)?;
    if reader.cursor != bytes.len() {
        return Err(invalid("trailing body data"));
    }
    let decoded = T::deserialize(Decoder(&value))?;
    // Reject unknown/defaulted fields, coercion and unsupported schema changes.
    if encode(&decoded)? != bytes {
        return Err(invalid("noncanonical typed body"));
    }
    Ok(decoded)
}

#[cfg(test)]
#[path = "reference_codec_tests.rs"]
mod tests;
