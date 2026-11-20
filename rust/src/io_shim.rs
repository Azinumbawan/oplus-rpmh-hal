// Read-only counter I/O. Test builds use only thread-local memory fixtures.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadError {
    Unavailable,
    InvalidData,
}

fn decode(bytes: Vec<u8>, limit: usize) -> Result<String, ReadError> {
    if bytes.len() > limit || bytes.contains(&0) {
        return Err(ReadError::InvalidData);
    }
    let text = String::from_utf8(bytes).map_err(|_| ReadError::InvalidData)?;
    if text
        .chars()
        .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
    {
        return Err(ReadError::InvalidData);
    }
    Ok(text.trim().to_owned())
}

#[cfg(not(test))]
pub fn read_file(path: &str, limit: usize) -> Result<String, ReadError> {
    use std::io::Read;
    let file = std::fs::File::open(path).map_err(|_| ReadError::Unavailable)?;
    let mut bytes = Vec::new();
    file.take(limit.checked_add(1).ok_or(ReadError::InvalidData)? as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| ReadError::Unavailable)?;
    decode(bytes, limit)
}

#[cfg(all(target_os = "android", not(test)))]
pub fn property_get(key: &str) -> Option<String> {
    use std::ffi::{CStr, CString};
    use std::os::raw::c_char;
    extern "C" {
        fn __system_property_get(name: *const c_char, value: *mut c_char) -> i32;
    }
    let key = CString::new(key).ok()?;
    let mut bytes = [0u8; 92];
    let len = unsafe { __system_property_get(key.as_ptr(), bytes.as_mut_ptr().cast()) };
    if len <= 0 || len as usize >= bytes.len() {
        return None;
    }
    Some(
        CStr::from_bytes_with_nul(&bytes[..=len as usize])
            .ok()?
            .to_str()
            .ok()?
            .to_owned(),
    )
}

#[cfg(all(not(target_os = "android"), not(test)))]
pub fn property_get(_key: &str) -> Option<String> {
    None
}

#[cfg(all(target_os = "android", target_pointer_width = "64", not(test)))]
pub fn boot_time_ms() -> Option<i64> {
    #[repr(C)]
    struct Timespec {
        seconds: i64,
        nanoseconds: i64,
    }
    extern "C" {
        fn clock_gettime(clock: i32, time: *mut Timespec) -> i32;
    }
    let mut time = Timespec {
        seconds: 0,
        nanoseconds: 0,
    };
    // CLOCK_BOOTTIME includes suspend; CLOCK_MONOTONIC does not.
    if unsafe { clock_gettime(7, &mut time) } != 0
        || time.seconds < 0
        || !(0..1_000_000_000).contains(&time.nanoseconds)
    {
        return None;
    }
    time.seconds
        .checked_mul(1000)?
        .checked_add(time.nanoseconds / 1_000_000)
}

#[cfg(all(
    not(all(target_os = "android", target_pointer_width = "64")),
    not(test)
))]
pub fn boot_time_ms() -> Option<i64> {
    None
}

#[cfg(test)]
thread_local! {
    static FILES: std::cell::RefCell<std::collections::HashMap<String, std::collections::VecDeque<Vec<u8>>>> = Default::default();
    static PROPS: std::cell::RefCell<std::collections::HashMap<String, String>> = Default::default();
    static CLOCK: std::cell::Cell<Option<i64>> = const { std::cell::Cell::new(None) };
    static READS: std::cell::RefCell<Vec<String>> = Default::default();
}

#[cfg(test)]
pub fn read_file(path: &str, limit: usize) -> Result<String, ReadError> {
    READS.with(|r| r.borrow_mut().push(path.to_owned()));
    let bytes = FILES
        .with(|f| {
            let mut files = f.borrow_mut();
            let values = files.get_mut(path)?;
            if values.len() > 1 {
                values.pop_front()
            } else {
                values.front().cloned()
            }
        })
        .ok_or(ReadError::Unavailable)?;
    decode(bytes, limit)
}
#[cfg(test)]
pub fn property_get(key: &str) -> Option<String> {
    PROPS.with(|p| p.borrow().get(key).cloned())
}
#[cfg(test)]
pub fn boot_time_ms() -> Option<i64> {
    CLOCK.with(|c| c.get())
}
#[cfg(test)]
pub fn mock_file(path: &str, text: &str) {
    mock_sequence(path, &[text]);
}
#[cfg(test)]
pub fn mock_sequence(path: &str, texts: &[&str]) {
    FILES.with(|f| {
        f.borrow_mut().insert(
            path.to_owned(),
            texts.iter().map(|s| s.as_bytes().to_vec()).collect(),
        )
    });
}
#[cfg(test)]
pub fn mock_property(key: &str, text: &str) {
    PROPS.with(|p| p.borrow_mut().insert(key.to_owned(), text.to_owned()));
}
#[cfg(test)]
pub fn mock_clock(time: i64) {
    CLOCK.with(|c| c.set(Some(time)));
}
#[cfg(test)]
pub fn mock_reads() -> Vec<String> {
    READS.with(|r| r.borrow().clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_utf8_only_and_no_host_fallback() {
        assert_eq!(decode(vec![0xff], 32), Err(ReadError::InvalidData));
        assert_eq!(decode(vec![0], 32), Err(ReadError::InvalidData));
        assert_eq!(decode(vec![b'x'; 33], 32), Err(ReadError::InvalidData));
        assert_eq!(
            read_file("/proc/this-test-does-not-read-host-proc", 32),
            Err(ReadError::Unavailable)
        );
    }
}
