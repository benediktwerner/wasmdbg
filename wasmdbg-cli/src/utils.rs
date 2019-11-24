use colored::*;
use terminal_size::{terminal_size, Width};

pub fn terminal_width() -> usize {
    match terminal_size() {
        Some((Width(w), _)) => w as usize,
        None => 80,
    }
}

pub fn print_header(text: &str) {
    let line_length = terminal_width() - text.len() - 8;
    println!(
        "{}",
        format!("──[ {} ]──{:─<2$}", text, "", line_length).blue()
    )
}

pub fn print_line() {
    println!("{}", format!("{:─<1$}", "", terminal_width()).blue());
}
