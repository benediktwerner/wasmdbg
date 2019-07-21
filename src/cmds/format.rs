use std::fmt::{Binary, Display, LowerHex, Octal};

use wasmdbg::value::WrapTo;

pub trait FormatSigned {
    fn fmt_signed(self) -> String;
}
macro_rules! impl_format_signed {
    ($u:ident, $s:ident) => {
        impl FormatSigned for $u {
            fn fmt_signed(self) -> String {
                format!("{}", self as $s)
            }
        }
    };
}
impl_format_signed!(u8, i8);
impl_format_signed!(u16, i16);
impl_format_signed!(u32, i32);
impl_format_signed!(u64, i64);

pub enum CharIter<'a> {
    Chars(std::str::Chars<'a>),
    Char(Option<char>),
    Vec(std::vec::IntoIter<char>),
}

impl<'a> Iterator for CharIter<'a> {
    type Item = char;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            CharIter::Chars(iter) => iter.next(),
            CharIter::Char(ref mut element) => {
                if let Some(c) = element {
                    let val = *c;
                    *element = None;
                    Some(val)
                } else {
                    None
                }
            }
            CharIter::Vec(iter) => iter.next(),
        }
    }
}

pub trait Formattable: Binary + LowerHex + Octal + Display + FormatSigned + WrapTo<u8> {
    fn fmt_float(self) -> String;
}

impl Formattable for u8 {
    fn fmt_float(self) -> String {
        self.fmt_signed()
    }
}
impl Formattable for u16 {
    fn fmt_float(self) -> String {
        self.fmt_signed()
    }
}
impl Formattable for u32 {
    fn fmt_float(self) -> String {
        format!("{:.8}", f32::from_bits(self))
    }
}
impl Formattable for u64 {
    fn fmt_float(self) -> String {
        format!("{:.16}", f64::from_bits(self))
    }
}

const HEX_CHARS: [char; 16] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
];

fn hex_char(b: u8) -> char {
    HEX_CHARS[b as usize]
}

pub fn fmt_char(b: &u8) -> CharIter {
    match *b {
        0 => CharIter::Chars(r"\0".chars()),
        0x7 => CharIter::Chars(r"\a".chars()),
        0x8 => CharIter::Chars(r"\b".chars()),
        0x9 => CharIter::Chars(r"\t".chars()),
        0xa => CharIter::Chars(r"\n".chars()),
        0xb => CharIter::Chars(r"\v".chars()),
        0xc => CharIter::Chars(r"\f".chars()),
        0xd => CharIter::Chars(r"\r".chars()),
        0x5c => CharIter::Chars(r"\\".chars()),
        c if 0x20 <= c && c < 0x7e => CharIter::Char(Some(c as char)),
        _ => {
            let v = vec!['\\', 'x', hex_char(b / 16), hex_char(b % 16)];
            CharIter::Vec(v.into_iter())
        }
    }
}

#[derive(Clone, Copy)]
pub enum Format {
    Decimal,
    Unsigned,
    Hex,
    Octal,
    Binary,
    Float,
    Char,
    Str,
}

impl Format {
    pub fn format<T: Formattable>(&self, val: T) -> String {
        let size = std::mem::size_of::<T>();
        match self {
            Format::Decimal => val.fmt_signed(),
            Format::Unsigned => format!("{}", val),
            Format::Hex => format!("0x{:01$x}", val, size * 2),
            Format::Octal => format!("0{:01$o}", val, size * 3),
            Format::Binary => format!("0b{:01$b}", val, size * 8),
            Format::Float => val.fmt_float(),
            Format::Char => format!("'{}'", fmt_char(&val.wrap_to()).collect::<String>()),
            Format::Str => String::default(),
        }
    }
}
