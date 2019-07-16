use std::any::Any;

use parity_wasm::elements::ValueType;
use crate::nan_preserving_float::{F32, F64};


#[derive(Clone)]
pub enum Value {
    I32(u32),
    I64(u64),
    F32(F32),
    F64(F64),
    V128(u128),
}

impl Value {
    pub fn default(value_type: ValueType) -> Self {
        match value_type {
            ValueType::I32 => Value::I32(0),
            ValueType::I64 => Value::I64(0),
            ValueType::F32 => Value::F32(F32::default()),
            ValueType::F64 => Value::F64(F64::default()),
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

impl From<u32> for Value {
    fn from(val: u32) -> Self {
        Value::I32(val)
    }
}
impl From<u64> for Value {
    fn from(val: u64) -> Self {
        Value::I64(val)
    }
}
impl From<f32> for Value {
    fn from(val: f32) -> Self {
        Value::F32(F32::from(val))
    }
}
impl From<f64> for Value {
    fn from(val: f64) -> Self {
        Value::F64(F64::from(val))
    }
}
impl From<F32> for Value {
    fn from(val: F32) -> Self {
        Value::F32(val)
    }
}
impl From<F64> for Value {
    fn from(val: F64) -> Self {
        Value::F64(val)
    }
}
impl From<u128> for Value {
    fn from(val: u128) -> Self {
        Value::V128(val)
    }
}

pub trait Number: Into<Value> + Copy + Any {
    fn value_type() -> ValueType;
}

macro_rules! impl_number {
    ($num_t:ident, $value_t:ident) => {
        impl Number for $num_t {
            fn value_type() -> ValueType {
                ValueType::$value_t
            }
        }
    };
}

impl_number!(u32, I32);
impl_number!(u64, I64);
impl_number!(f32, F32);
impl_number!(f64, F64);
impl_number!(F32, F32);
impl_number!(F64, F64);
impl_number!(u128, V128);


pub trait Integer<T> {
    fn leading_zeros(self) -> T;
    fn trailing_zeros(self) -> T;
    fn count_ones(self) -> T;
    fn rotl(self, other: T) -> T;
    fn rotr(self, other: T) -> T;
    fn rem(self, other: T) -> T;
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

macro_rules! impl_little_endian_convert_int {
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

macro_rules! impl_little_endian_convert_float {
    ($t:ident, $repr:ident) => {
        impl LittleEndianConvert for $t {
            fn from_little_endian(buffer: &[u8]) -> Self {
                Self::from_bits($repr::from_little_endian(buffer))
            }

            fn to_little_endian(self, buffer: &mut [u8]) {
                self.to_bits().to_little_endian(buffer);
            }
        }
    };
}

impl_little_endian_convert_int!(i16);
impl_little_endian_convert_int!(u16);
impl_little_endian_convert_int!(i32);
impl_little_endian_convert_int!(u32);
impl_little_endian_convert_int!(i64);
impl_little_endian_convert_int!(u64);
impl_little_endian_convert_float!(f32, u32);
impl_little_endian_convert_float!(f64, u64);
impl_little_endian_convert_float!(F32, u32);
impl_little_endian_convert_float!(F64, u64);


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
