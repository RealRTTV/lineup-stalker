use core::ffi::c_void;

#[repr(C)]
pub struct ConsoleCursorInfo {
    size: i32,
    visible: u32,
}

impl ConsoleCursorInfo {
    pub const fn new(size: i32, visible: bool) -> Self {
        Self {
            size,
            visible: visible as u32,
        }
    }
}

#[repr(C)]
pub struct SystemTime {
    pub year: u16,
    pub month: u16,
    pub day_of_week: u16,
    pub day: u16,
    pub hour: u16,
    pub minute: u16,
    pub second: u16,
    pub milliseconds: u16,
}

#[repr(C)]
pub struct TimeZoneInformation {
    pub bias: u32,
    pub standard_name: [u16; 32],
    pub standard_date: SystemTime,
    pub standard_bias: u32,
    pub daylight_name: [u16; 32],
    pub daylight_date: SystemTime,
    pub daylight_bias: u32,
}

#[repr(C)]
pub struct Coordinate {
    pub x: i16,
    pub y: i16,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    pub fn SetConsoleCursorInfo(handle: *mut c_void, param: *const ConsoleCursorInfo) -> bool;

    pub fn SetConsoleCursorPosition(handle: *mut c_void, pos: Coordinate) -> bool;

    pub fn SetConsoleTextAttribute(handle: *mut c_void, attributes: u16) -> bool;

    #[must_use]
    pub fn GetStdHandle(id: u32) -> *mut c_void;

    #[must_use]
    pub fn GetConsoleWindow() -> *mut c_void;

    #[must_use]
    pub fn GetTimeZoneInformation(ptr: *mut TimeZoneInformation) -> i32;
}

#[link(name = "msvcrt")]
unsafe extern "system" {
    pub fn _getch() -> u32;
}

#[link(name = "user32")]
unsafe extern "system" {
    pub fn SetForegroundWindow(hwnd: *mut c_void) -> bool;
}

pub fn flush() {
    let _ = std::io::Write::flush(&mut std::io::stdout());
}

#[must_use]
pub fn read_char() -> u32 {
    unsafe { _getch() }
}

pub fn set_cursor(x: usize, y: usize) {
    unsafe { SetConsoleCursorPosition(GetStdHandle(-11_i32 as u32), Coordinate { x: x as i16, y: y as i16 }); }
}

pub fn set_cursor_visible(visible: bool) {
    unsafe { SetConsoleCursorInfo(GetStdHandle(-11_i32 as u32), &ConsoleCursorInfo::new(1, visible)); }
}

pub fn set_text_attribute(attributes: u16) {
    unsafe { SetConsoleTextAttribute(GetStdHandle(-11_i32 as u32), attributes); }
}