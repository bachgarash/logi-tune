//! HID++ 2.0 report construction, sending, and response parsing.

use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::io::AsRawFd;

use nix::poll::{PollFd, PollFlags};

use super::{HidError, REPORT_ID_LONG, REPORT_ID_SHORT};

/// Wait up to `timeout_ms` for the file descriptor to become readable.
/// Returns `false` if the timeout expires with no data available.
fn wait_readable(file: &File, timeout_ms: i32) -> bool {
    let fd = file.as_raw_fd();
    let mut fds = [PollFd::new(
        unsafe { std::os::fd::BorrowedFd::borrow_raw(fd) },
        PollFlags::POLLIN,
    )];
    nix::poll::poll(
        &mut fds,
        nix::poll::PollTimeout::try_from(timeout_ms).unwrap_or(nix::poll::PollTimeout::ZERO),
    )
    .unwrap_or(0)
        > 0
}

/// Software ID embedded in every report (used to correlate responses).
/// Must be non-zero; 0x01 is the conventional value used by third-party tools.
const SW_ID: u8 = 0x01;

/// Send a 7-byte short HID++ report and read back a 7-byte response.
pub fn send_short(
    file: &mut File,
    device_index: u8,
    feature_index: u8,
    func: u8,
    params: [u8; 3],
) -> Result<[u8; 7], HidError> {
    let report: [u8; 7] = [
        REPORT_ID_SHORT,
        device_index,
        feature_index,
        (func << 4) | SW_ID,
        params[0],
        params[1],
        params[2],
    ];

    tracing::debug!(tx = ?report, "send_short");
    file.write_all(&report)?;

    // Drain non-HID++ reports (e.g. mouse movement) that may arrive before
    // the response on BT/UHID interfaces.  Each read is guarded by a 200 ms
    // poll timeout so we never block forever when the kernel driver consumes
    // the response (as it does over BT).
    let mut response = [0u8; 7];
    for _ in 0..16 {
        if !wait_readable(file, 200) {
            return Err(HidError::Protocol(0xff)); // timeout
        }
        file.read_exact(&mut response)?;
        tracing::debug!(rx = ?response, "send_short rx");
        if response[0] == REPORT_ID_SHORT || response[0] == REPORT_ID_LONG {
            break;
        }
    }

    check_short_error(&response)?;
    Ok(response)
}

/// Send a 20-byte long HID++ report and read back a 20-byte response.
pub fn send_long(
    file: &mut File,
    device_index: u8,
    feature_index: u8,
    func: u8,
    params: [u8; 16],
) -> Result<[u8; 20], HidError> {
    let mut report = [0u8; 20];
    report[0] = REPORT_ID_LONG;
    report[1] = device_index;
    report[2] = feature_index;
    report[3] = (func << 4) | SW_ID;
    report[4..20].copy_from_slice(&params);

    tracing::debug!(tx = ?report, "send_long");
    file.write_all(&report)?;

    let mut response = [0u8; 20];
    for _ in 0..16 {
        if !wait_readable(file, 200) {
            return Err(HidError::Protocol(0xff)); // timeout
        }
        file.read_exact(&mut response)?;
        tracing::debug!(rx = ?response, "send_long rx");
        if response[0] == REPORT_ID_SHORT || response[0] == REPORT_ID_LONG {
            break;
        }
    }

    check_long_error(&response)?;
    Ok(response)
}

/// Check a short response for a HID++ error report (feature_index == 0x8f).
fn check_short_error(response: &[u8; 7]) -> Result<(), HidError> {
    // Byte 2 == 0x8f signals an error; byte 4 carries the error code.
    if response[2] == 0x8f {
        return Err(HidError::Protocol(response[4]));
    }
    Ok(())
}

/// Check a long response for a HID++ error report (feature_index == 0x8f).
fn check_long_error(response: &[u8; 20]) -> Result<(), HidError> {
    if response[2] == 0x8f {
        return Err(HidError::Protocol(response[4]));
    }
    Ok(())
}
