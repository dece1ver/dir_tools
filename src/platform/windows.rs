use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

use winapi::um::fileapi::INVALID_FILE_ATTRIBUTES;
use winapi::um::fileapi::SetFileAttributesW;
use winapi::um::winnt::FILE_ATTRIBUTE_HIDDEN;

pub fn remove_hidden_attribute(path: &str) -> bool {
    let wide: Vec<u16> = OsStr::new(path).encode_wide().chain(Some(0)).collect();
    unsafe {
        let attributes = winapi::um::fileapi::GetFileAttributesW(wide.as_ptr());
        if attributes != INVALID_FILE_ATTRIBUTES && (attributes & FILE_ATTRIBUTE_HIDDEN) != 0 {
            SetFileAttributesW(wide.as_ptr(), attributes & !FILE_ATTRIBUTE_HIDDEN);
            true
        } else {
            false
        }
    }
}
