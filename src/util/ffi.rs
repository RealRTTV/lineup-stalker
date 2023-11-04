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
extern "system" {
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
extern "system" {
    pub fn _getch() -> u32;
}

#[link(name = "user32")]
extern "system" {
    pub fn SetForegroundWindow(hwnd: *mut c_void) -> bool;
}
