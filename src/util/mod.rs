use core::fmt::Write;
use std::ffi::CStr;
use crate::util::ffi::{Coordinate, GetStdHandle, GetTimeZoneInformation, SetConsoleCursorPosition};

pub mod ffi;
pub mod statsapi;
pub mod record_against;
pub mod standings;

pub fn nth(n: usize) -> String {
    let mut buf = String::with_capacity(n.checked_ilog10().map_or(1, |x| x + 1) as usize + 2);
    let _ = write!(&mut buf, "{n}");
    if n / 10 % 10 == 1 {
        buf.push_str("th");
    } else {
        match n % 10 {
            1 => buf.push_str("st"),
            2 => buf.push_str("nd"),
            3 => buf.push_str("rd"),
            _ => buf.push_str("th"),
        }
    }
    buf
}

pub fn last_name(s: &str) -> &str {
    s.rsplit_once(' ').map_or(s, |x| x.1)
}

pub fn clear_screen(height: usize) {
    let handle = unsafe { GetStdHandle(-11_i32 as u32) };
    for n in 0..height {
        unsafe {
            SetConsoleCursorPosition(handle, Coordinate { x: 0, y: n as i16 });
        }
        println!("{}", unsafe {
            core::str::from_utf8_unchecked(&[b' '; 1024])
        });
    }
}

pub fn hide(s: &str) -> String {
    s.chars().map(|x| if x.is_ascii_whitespace() { " " } else { r"\_" }).collect::<String>()
}

// not possible without windows
pub fn get_local_team() -> Option<&'static str> {
    let mut time_zone_information = core::mem::MaybeUninit::uninit();
    let kind = unsafe { GetTimeZoneInformation(time_zone_information.as_mut_ptr()) };
    let time_zone_information = unsafe { time_zone_information.assume_init() };
    let name_wide = if kind == 1 { time_zone_information.standard_name } else { time_zone_information.daylight_name };
    let name = {
        let mut array = [0_u8; 32];
        for (idx, &val) in name_wide.iter().enumerate() {
            array[idx] = val as u8;
        }
        array
    };
    let _timezone = CStr::from_bytes_until_nul(&name).ok()?.to_str().ok()?;
    Some("Toronto Blue Jays")
}
