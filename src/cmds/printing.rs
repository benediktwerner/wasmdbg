use std::convert::{TryFrom, TryInto};
use std::fmt::{Binary, Display, LowerHex, Octal};

use failure::Error;

use wasmdbg::value::WrapTo;
use wasmdbg::Debugger;

use super::{CmdArg, CmdResult, Command, Commands};

pub fn add_cmds(commands: &mut Commands) {
    commands.add(
        Command::new("x", cmd_x)
            .takes_args("/FMT ADDRESS:str")
            .description("Examine memory")
            .requires_running(),
    );
}

trait FormatSigned {
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

enum CharIter<'a> {
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

trait Formattable: Binary + LowerHex + Octal + Display + FormatSigned + WrapTo<u8> {
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

fn fmt_char(b: &u8) -> CharIter {
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

enum Format {
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
    fn format<T: Formattable>(&self, val: T) -> String {
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

impl TryFrom<char> for Format {
    type Error = ();

    fn try_from(c: char) -> Result<Self, Self::Error> {
        Ok(match c {
            'd' => Format::Decimal,
            'u' => Format::Unsigned,
            'x' => Format::Hex,
            'o' => Format::Octal,
            'b' => Format::Binary,
            'f' => Format::Float,
            'c' => Format::Char,
            's' => Format::Str,
            _ => return Err(()),
        })
    }
}

fn cmd_x(dbg: &mut Debugger, args: &[CmdArg]) -> CmdResult {
    let ((count, size, format), address) = if args.len() == 2 {
        (parse_format(&args[0].as_str())?, args[1].as_str())
    } else {
        ((1, 4, Format::Hex), args[0].as_str())
    };
    let memory = dbg.memory()?;
    let address = parse_address(&address).map_err(|e| format_err!("Invalid address: {}", e))?;
    let mut addr = address;
    for _ in 0..count {
        if let Format::Str = format {
            let bytes: Vec<u8> = memory.data()[addr as usize..]
                .iter()
                .cloned()
                .take_while(|b| *b != 0)
                .collect();
            let val_str: String = bytes.iter().flat_map(|b| fmt_char(b)).collect();
            println!(" 0x{:08x}: \"{}\"", addr, val_str);
            addr += bytes.len() as u32 + 1;
        } else {
            let val_str = match size {
                1 => format.format(memory.load::<u8>(addr)?),
                2 => format.format(memory.load::<u16>(addr)?),
                4 => format.format(memory.load::<u32>(addr)?),
                8 => format.format(memory.load::<u64>(addr)?),
                _ => unreachable!(),
            };
            println!(" 0x{:08x}: {}", addr, val_str);
            addr += count * size;
        }
    }
    Ok(())
}

fn parse_address(addr: &str) -> Result<u32, std::num::ParseIntError> {
    if addr.len() > 2 && addr[0..2].to_lowercase() == "0x" {
        u32::from_str_radix(&addr[2..], 16)
    } else {
        u32::from_str_radix(addr, 10)
    }
}

fn parse_format(fmt_str: &str) -> Result<(u32, u32, Format), Error> {
    let count_str = fmt_str
        .chars()
        .take_while(|c| c.is_numeric())
        .collect::<String>();
    let count = if count_str.is_empty() {
        1
    } else {
        count_str.parse()?
    };
    let mut size = 4;
    let mut format = Format::Decimal;
    for c in fmt_str.chars().skip_while(|c| c.is_numeric()) {
        match c {
            'b' => size = 1,
            'h' => size = 2,
            'w' => size = 4,
            'g' => size = 8,
            _ => {
                if let Ok(fmt) = c.try_into() {
                    format = fmt;
                }
            }
        }
    }
    if let Format::Char = format {
        size = 1;
    }
    Ok((count, size, format))
}
