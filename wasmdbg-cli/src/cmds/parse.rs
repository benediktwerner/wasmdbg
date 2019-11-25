use std::convert::{TryFrom, TryInto};
use std::fmt;

use wasmdbg::value::Integer;

use super::format::Format;
use super::{CmdArg, CmdArgType};

pub fn parse_types(line: &'static str) -> Vec<CmdArgType> {
    let mut last_index = 0;
    let mut size = 0;
    let mut bracket_level = 0;
    let mut result = Vec::new();

    for c in line.chars() {
        match c {
            '[' => bracket_level += 1,
            ']' => bracket_level -= 1,
            c if c.is_whitespace() && bracket_level == 0 => {
                let curr_index = last_index + size;
                result.push(line[last_index..curr_index].into());
                last_index = curr_index + c.len_utf8();
                size = 0;
                continue;
            }
            _ => (),
        }
        size += c.len_utf8();
    }

    if bracket_level != 0 {
        panic!("Unmachted '[' in cmd args: {}", line);
    }

    result.push(line[last_index..].into());
    result
}

impl fmt::Display for CmdArgType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CmdArgType::Str(name) => write!(f, "{}", name),
            CmdArgType::Fmt(name) => write!(f, "/{}", name),
            CmdArgType::Path(name) => write!(f, "{}", name),
            CmdArgType::Line(name) => write!(f, "{}", name),
            CmdArgType::Usize(name) => write!(f, "{}", name),
            CmdArgType::U32(name) => write!(f, "{}", name),
            CmdArgType::Addr(name) => write!(f, "{}", name),
            CmdArgType::Const(val) => write!(f, "{}", val),
            CmdArgType::Union(elements) => write!(
                f,
                "{}",
                elements
                    .iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<String>>()
                    .join("|")
            ),
            CmdArgType::List(arg_type) => write!(f, "{}...", arg_type),
            CmdArgType::Opt(arg_types) => write!(
                f,
                "[{}]",
                arg_types
                    .iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<String>>()
                    .join(" ")
            ),
        }
    }
}

impl From<&'static str> for CmdArgType {
    fn from(s: &'static str) -> Self {
        if s.starts_with('[') {
            CmdArgType::Opt(parse_types(&s[1..s.len() - 1]))
        } else if s.ends_with("...") {
            CmdArgType::List(Box::new(s[..s.len() - 3].into()))
        } else if s.contains('|') {
            CmdArgType::Union(s.split('|').map(|a| a.into()).collect())
        } else if s.contains(':') {
            let mut split = s.split(':');
            let name = split.next().unwrap();
            match split.next().unwrap() {
                "str" => CmdArgType::Str(name),
                "path" => CmdArgType::Path(name),
                "line" => CmdArgType::Line(name),
                "usize" => CmdArgType::Usize(name),
                "u32" => CmdArgType::U32(name),
                "addr" => CmdArgType::Addr(name),
                other => panic!("Invalid type in cmd arguments: {}", other),
            }
        } else if s.starts_with('/') {
            CmdArgType::Fmt(&s[1..])
        } else {
            CmdArgType::Const(s)
        }
    }
}

pub trait ParseCmdArg {
    fn parse<'a>(&self, line: &'a str) -> anyhow::Result<(&'a str, Vec<CmdArg>)>;
    fn parse_all(&self, line: &str) -> anyhow::Result<Vec<CmdArg>> {
        match self.parse(line) {
            Ok(("", arg)) => Ok(arg),
            Ok(_) => bail!("Too many arguments."),
            Err(error) => Err(error),
        }
    }
}

impl ParseCmdArg for CmdArgType {
    fn parse<'a>(&self, line: &'a str) -> Result<(&'a str, Vec<CmdArg>), Error> {
        match self {
            CmdArgType::Str(_) | CmdArgType::Path(_) => {
                wrap(next_arg(line), |a| Ok(CmdArg::Str(a.to_string())))
            }
            CmdArgType::Line(_) => Ok(("", vec![CmdArg::Str(line.to_string())])),
            CmdArgType::Fmt(_) => {
                if let Some('/') = line.trim_start().chars().next() {
                    wrap(next_arg(&line.trim_start()[1..]), |a| {
                        let (count, size, format) = parse_format(a)?;
                        Ok(CmdArg::Fmt(count, size, format))
                    })
                } else {
                    Ok((line, Vec::with_capacity(0)))
                }
            }
            CmdArgType::Usize(_) => wrap(next_arg(line), |a| Ok(CmdArg::Usize(a.parse()?))),
            CmdArgType::U32(_) => wrap(next_arg(line), |a| Ok(CmdArg::U32(a.parse()?))),
            CmdArgType::Addr(_) => wrap(next_arg(line), |a| {
                Ok(CmdArg::U32(u32::from_str_with_radix(a)?))
            }),
            CmdArgType::Const(val) => {
                if line.trim_start().starts_with(*val) {
                    Ok((&line[val.len()..], vec![CmdArg::Const(val)]))
                } else {
                    bail!("Expected \"{}\"", val);
                }
            }
            CmdArgType::Union(elements) => {
                for e in elements.iter() {
                    if let Ok(arg) = e.parse(line) {
                        return Ok(arg);
                    }
                }
                bail!("Expected {}", self);
            }
            CmdArgType::List(arg_type) => {
                let mut line = line;
                let mut result = Vec::new();
                while !line.trim().is_empty() {
                    let (rest, args) = match arg_type.parse(line) {
                        Ok(result) => result,
                        Err(error) => bail!("Invalid arguments: \"{}\". {}", line, error),
                    };
                    result.extend(args);
                    line = rest;
                }
                Ok(("", result))
            }
            CmdArgType::Opt(arg_types) => {
                if line.trim().is_empty() {
                    Ok(("", Vec::with_capacity(0)))
                } else {
                    arg_types.parse(line)
                }
            }
        }
    }
}

impl ParseCmdArg for Vec<CmdArgType> {
    fn parse<'a>(&self, mut line: &'a str) -> anyhow::Result<(&'a str, Vec<CmdArg>)> {
        let mut result = Vec::new();
        for arg_type in self {
            match arg_type.parse(line) {
                Ok((rest, args)) => {
                    result.extend(args);
                    line = rest;
                }
                Err(error) => {
                    if line.is_empty() {
                        bail!("Missing argument(s)");
                    }
                    bail!("Invalid argument: \"{}\". {}", line, error);
                }
            }
        }
        Ok((line, result))
    }
}

fn wrap<'a, F>(arg: anyhow::Result<(&'a str, &str)>, f: F) -> anyhow::Result<(&'a str, Vec<CmdArg>)>
where
    F: Fn(&str) -> anyhow::Result<CmdArg>,
{
    let (rest, arg) = arg?;
    Ok((rest, vec![f(arg)?]))
}

fn next_arg(line: &str) -> anyhow::Result<(&str, &str)> {
    let mut iter = line.trim_start().splitn(2, char::is_whitespace);
    if let Some(arg) = iter.next() {
        if let Some(rest) = iter.next() {
            return Ok((rest, arg));
        } else if !arg.is_empty() {
            return Ok(("", arg));
        }
    }
    bail!("Missig argument(s)")
}

// fn next_arg_escaped(line: &str) -> anyhow::Result<(&str, &str)> {
//     let line = line.trim_start();

//     if line.is_empty() {
//         return Err(err_msg("Missing argument(s)"));
//     }

//     let mut chars = line.chars();
//     let mut quote = match chars.next().unwrap() {
//         c if c == '"' || c == '\'' => Some(c),
//         _ => None,
//     };

//     if let Some(quote_char) = quote {
//         let mut escape = false;
//         for c in chars.by_ref() {
//             if c == quote_char && !escape {
//                 quote = None;
//                 break;
//             }
//             if !escape && c == '\\' {
//                 escape = true;
//             } else {
//                 escape = false;
//             }
//         }
//         if quote.is_some() {
//             return Err(err_msg("Unmatched quote in arguments"));
//         }
//         if let Some(c) = chars.next() {
//             if !c.is_whitespace() {
//                 return Err(err_msg("Expected whitespace after closing quote"));
//             }
//         }
//     } else {
//         for c in chars.by_ref() {
//             if c.is_whitespace() {
//                 break;
//             }
//         }
//     }

//     let rest = chars.as_str();
//     let arg_len = line.len() - rest.len() - 1;
//     Ok((rest, &line[..arg_len]))
// }

fn parse_format(fmt_str: &str) -> anyhow::Result<(u32, u32, Format)> {
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
    let mut format = Format::Hex;
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

impl TryFrom<char> for Format {
    type Error = ();

    fn try_from(c: char) -> Result<Self, Self::Error> {
        Ok(match c {
            'd' => Format::Decimal,
            'u' => Format::Unsigned,
            'x' => Format::Hex,
            'o' => Format::Octal,
            't' => Format::Binary,
            'f' => Format::Float,
            'c' => Format::Char,
            's' => Format::Str,
            _ => return Err(()),
        })
    }
}
