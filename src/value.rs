use std::any::Any;

use parity_wasm::elements::ValueType;

#[derive(Clone)]
pub enum Value {
    I32(u32),
    I64(u64),
    F32(f32),
    F64(f64),
    V128(u128),
}

impl Value {
    pub fn default(value_type: ValueType) -> Self {
        match value_type {
            ValueType::I32 => Value::I32(0),
            ValueType::I64 => Value::I64(0),
            ValueType::F32 => Value::F32(0.0),
            ValueType::F64 => Value::F64(0.0),
            ValueType::V128 => Value::V128(0),
        }
    }

    pub fn value_type(&self) -> ValueType {
        match self {
            Value::I32(_) => ValueType::I32,
            Value::I64(_) => ValueType::I64,
            Value::F32(_) => ValueType::F32,
            Value::F64(_) => ValueType::F64,
            Value::V128(_) => ValueType::V128,
        }
    }

    pub fn value_as_any(&self) -> &Any {
        match self {
            Value::I32(ref val) => val,
            Value::I64(ref val) => val,
            Value::F32(ref val) => val,
            Value::F64(ref val) => val,
            Value::V128(ref val) => val,
        }
    }
}

pub trait Num: Copy + Any {
    fn value_type() -> ValueType;
    fn to_value(self) -> Value;
}
impl Num for u32 {
    fn value_type() -> ValueType {
        ValueType::I32
    }
    fn to_value(self) -> Value {
        Value::I32(self)
    }
}
impl Num for u64 {
    fn value_type() -> ValueType {
        ValueType::I64
    }
    fn to_value(self) -> Value {
        Value::I64(self)
    }
}
impl Num for f32 {
    fn value_type() -> ValueType {
        ValueType::F32
    }
    fn to_value(self) -> Value {
        Value::F32(self)
    }
}
impl Num for f64 {
    fn value_type() -> ValueType {
        ValueType::F64
    }
    fn to_value(self) -> Value {
        Value::F64(self)
    }
}
impl Num for u128 {
    fn value_type() -> ValueType {
        ValueType::V128
    }
    fn to_value(self) -> Value {
        Value::V128(self)
    }
}

pub trait LittleEndianConvert: Sized {
    fn from_little_endian(buffer: &[u8]) -> Self;
    fn to_little_endian(self, buffer: &mut [u8]);
}
impl LittleEndianConvert for i8 {
    fn from_little_endian(buffer: &[u8]) -> Self {
        buffer[0] as i8
    }

    fn to_little_endian(self, buffer: &mut [u8]) {
        buffer[0] = self as u8;
    }
}
impl LittleEndianConvert for u8 {
    fn from_little_endian(buffer: &[u8]) -> Self {
        buffer[0]
    }

    fn to_little_endian(self, buffer: &mut [u8]) {
        buffer[0] = self;
    }
}
impl LittleEndianConvert for f32 {
    fn from_little_endian(buffer: &[u8]) -> Self {
        let mut buf = [0u8; 4];
        buf.copy_from_slice(&buffer[0..4]);
        Self::from_bits(u32::from_le_bytes(buf))
    }

    fn to_little_endian(self, buffer: &mut [u8]) {
        buffer.copy_from_slice(&self.to_bits().to_le_bytes());
    }
}
impl LittleEndianConvert for f64 {
    fn from_little_endian(buffer: &[u8]) -> Self {
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&buffer[0..8]);
        Self::from_bits(u64::from_le_bytes(buf))
    }

    fn to_little_endian(self, buffer: &mut [u8]) {
        buffer.copy_from_slice(&self.to_bits().to_le_bytes());
    }
}

macro_rules! impl_little_endian_convert {
    ($t:ident) => {
        impl LittleEndianConvert for $t {
            fn from_little_endian(buffer: &[u8]) -> Self {
                const SIZE: usize = core::mem::size_of::<$t>();
                let mut buf = [0u8; SIZE];
                buf.copy_from_slice(&buffer[0..SIZE]);
                Self::from_le_bytes(buf)
            }

            fn to_little_endian(self, buffer: &mut [u8]) {
                buffer.copy_from_slice(&self.to_le_bytes());
            }
        }
    };
}

impl_little_endian_convert!(i16);
impl_little_endian_convert!(u16);
impl_little_endian_convert!(i32);
impl_little_endian_convert!(u32);
impl_little_endian_convert!(i64);
impl_little_endian_convert!(u64);


pub trait ExtendTo<T> {
    fn extend_to(self) -> T;
}

macro_rules! impl_extend_to {
    ($from:ident, $to:ident) => {
        impl ExtendTo<$to> for $from {
            fn extend_to(self) -> $to {
                self as $to
            }
        }
    };
}

impl_extend_to!(i8, u32);
impl_extend_to!(u8, u32);
impl_extend_to!(u16, u32);
impl_extend_to!(i16, u32);
impl_extend_to!(i8, u64);
impl_extend_to!(u8, u64);
impl_extend_to!(i16, u64);
impl_extend_to!(u16, u64);
impl_extend_to!(i32, u64);
impl_extend_to!(u32, u64);
impl_extend_to!(f32, f64);

pub trait WrapTo<T> {
    fn wrap_to(self) -> T;
}

macro_rules! impl_wrap_to {
    ($from:ident, $to:ident) => {
        impl WrapTo<$to> for $from {
            fn wrap_to(self) -> $to {
                self as $to
            }
        }
    };
}

impl_wrap_to!(u32, u8);
impl_wrap_to!(u32, u16);
impl_wrap_to!(u64, u8);
impl_wrap_to!(u64, u16);
impl_wrap_to!(u64, u32);
