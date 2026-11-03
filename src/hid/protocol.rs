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
///
/// Must be non-zero.  We deliberately avoid `0x01`: that value is in common
/// use by other HID++ clients that may be talking to the same device, and the
/// software ID is the only thing distinguishing our replies from theirs.
const SW_ID: u8 = 0x0a;

/// Longest HID++ report (very-long reports are 64 bytes).
const MAX_REPORT: usize = 64;

/// Outcome of inspecting one received report.
enum Match {
    /// The reply to our request.
    Ours,
    /// A HID++ error report carrying this error code.
    Error(u8),
    /// Someone else's reply, or a device notification — keep waiting.
    Unrelated,
}

/// Classify a received report against the request we sent.
///
/// A response echoes the feature index in byte 2 and `(func << 4) | SW_ID` in
/// byte 3.  Matching on both is what makes this safe to use while the device
/// is also emitting notifications (button presses, wheel ratchet changes) and
/// while other HID++ clients are active.
fn classify(report: &[u8], feature_index: u8, func: u8) -> Match {
    if report.len() < 4 || (report[0] != REPORT_ID_SHORT && report[0] != REPORT_ID_LONG) {
        return Match::Unrelated;
    }
    // HID++ 2.0 error: byte 2 == 0xff, byte 3 echoes our func/SW_ID, byte 5 is the code.
    if report[2] == 0xff && report.len() >= 6 && (report[3] & 0x0f) == SW_ID {
        return Match::Error(report[5]);
    }
    // HID++ 1.0 error: byte 2 == 0x8f, byte 4 is the code.
    if report[2] == 0x8f && report.len() >= 5 {
        return Match::Error(report[4]);
    }
    if report[2] == feature_index && report[3] == (func << 4) | SW_ID {
        return Match::Ours;
    }
    Match::Unrelated
}

/// Send `request` and wait for the matching reply, discarding notifications
/// and other clients' traffic.
///
/// Reads are sized to the largest possible report rather than to the request
/// size: the device may answer a long request with a short report, and
/// `read_exact` on an undersized buffer would stall waiting for bytes that
/// never arrive.
fn exchange(
    file: &mut File,
    request: &[u8],
    feature_index: u8,
    func: u8,
) -> Result<[u8; MAX_REPORT], HidError> {
    tracing::debug!(tx = ?request, "hidpp tx");
    file.write_all(request)?;

    let mut buf = [0u8; MAX_REPORT];
    for _ in 0..32 {
        if !wait_readable(file, 300) {
            return Err(HidError::Timeout);
        }
        let n = file.read(&mut buf)?;
        let report = &buf[..n];
        match classify(report, feature_index, func) {
            Match::Ours => {
                tracing::debug!(rx = ?report, "hidpp rx");
                let mut out = [0u8; MAX_REPORT];
                out[..n].copy_from_slice(report);
                return Ok(out);
            }
            Match::Error(code) => {
                tracing::debug!(rx = ?report, code, "hidpp error");
                return Err(HidError::Protocol(code));
            }
            Match::Unrelated => continue,
        }
    }
    Err(HidError::Timeout)
}

/// Send a 7-byte short HID++ report and read back the response.
///
/// Only valid on transports that declare report ID `0x10`; see `HidBus`.
pub fn send_short(
    file: &mut File,
    device_index: u8,
    feature_index: u8,
    func: u8,
    params: [u8; 3],
) -> Result<[u8; MAX_REPORT], HidError> {
    let report: [u8; 7] = [
        REPORT_ID_SHORT,
        device_index,
        feature_index,
        (func << 4) | SW_ID,
        params[0],
        params[1],
        params[2],
    ];
    exchange(file, &report, feature_index, func)
}

/// Send a 20-byte long HID++ report and read back the response.
///
/// This is the only report size available over Bluetooth.
pub fn send_long(
    file: &mut File,
    device_index: u8,
    feature_index: u8,
    func: u8,
    params: [u8; 16],
) -> Result<[u8; MAX_REPORT], HidError> {
    let mut report = [0u8; 20];
    report[0] = REPORT_ID_LONG;
    report[1] = device_index;
    report[2] = feature_index;
    report[3] = (func << 4) | SW_ID;
    report[4..20].copy_from_slice(&params);
    exchange(file, &report, feature_index, func)
}
