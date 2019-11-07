//!
//! Example
//!
//! ```rust
//! use shortid::*;
//!
//! fn to_string(src:&[u8])->String {
//!     src
//!         .into_iter()
//!         .map(|val| format!("{:0>2x}", val))
//!         .collect()
//! }
//!
//! fn main() {
//!
//!     let mac = [1,2,3,4,5,6];
//!     let epoch = 0;
//!
//!     println!("{}" , to_string(&uuidv1(mac).unwrap()));
//!
//!     let mac = [1,2,3,4];
//!     println!("{}" , to_string(&next_short_128(mac).unwrap()));
//!
//!     let mac = [1,2,3];
//!     println!("{}" , to_string(&next_short_96(mac,epoch).unwrap()));
//!
//!     println!("{}" , to_string(&next_short_64(epoch).unwrap()));
//!
//! }
//! ```

use ::failure::Fail;

#[derive(Fail, Debug)]
pub enum Error {
    #[fail(display = "There is not enough ID now")]
    TimeOverflow,
    #[fail(display = "SystemTime before UNIX EPOCH!")]
    SystemTimeException,
    #[fail(display = "Too many threads")]
    WorkerIDOverflow,
    #[fail(display = "SystemTime before EPOCH!")]
    EpochException,
}

use std::cell::RefCell;
use std::sync::atomic::{AtomicUsize, Ordering};

const UUID_TICKS_BETWEEN_EPOCHS: u64 = 0x01B2_1DD2_1381_4000;
const TIMESTAMP42SHIFT: u8 = 13;

static mut COUNTER: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static WORKER_ID: [u8;2] = {
        unsafe{
            let id = COUNTER.fetch_add(1, Ordering::SeqCst);
            if id > u16::max_value() as usize {
                panic!("too many threads")
            };
            (id as u16).to_be_bytes()
        }
    };
    static SEQ: RefCell<u16> = RefCell::new(0);
    static TIMESTAMP: RefCell<u64> = RefCell::new(now().unwrap());
}

//100ns since unix_epoch;
fn now() -> Result<u64, Error> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Error::SystemTimeException)?;
    Ok((time.as_nanos() / 100) as u64 + UUID_TICKS_BETWEEN_EPOCHS)
}

fn worker_id() -> [u8; 2] {
    WORKER_ID.with(|f| *f)
}

fn time_inc(min_interval: u16) -> Result<u64, Error> {
    TIMESTAMP.with(|t| {
        let mut time = t.borrow_mut();
        if cfg!(test) && (*time) >= now()? {
            return Err(Error::TimeOverflow);
        }
        *time += u64::from(min_interval);
        Ok(*time)
    })
}

fn next(min_interval: u16) -> Result<(u64, u16), Error> {
    SEQ.with(|s| {
        let mut seq = s.borrow_mut();
        if *seq < ((1 << 14) - 1) {
            *seq += 1;
            Ok((timestamp(), *seq))
        } else {
            let t = time_inc(min_interval)?;
            *seq = 0;
            Ok((t, 0))
        }
    })
}

fn timestamp() -> u64 {
    TIMESTAMP.with(|t| *t.borrow())
}

#[cfg(test)]
fn seq() -> u16 {
    SEQ.with(|s| *s.borrow())
}

///
/// for compatible UUID
///
/// 16 bit worker id and 24 bit machine_id
///
pub fn next_short_128(machine_id: [u8; 4]) -> Result<[u8; 16], Error> {
    let (t, s) = next(1)?;
    let w = worker_id();
    let time_low = ((t & 0xFFFF_FFFF) as u32).to_be_bytes();
    let time_mid = (((t >> 32) & 0xFFFF) as u16).to_be_bytes();
    let time_high_and_version = ((((t >> 48) & 0x0FFF) as u16) | (1 << 12)).to_be_bytes();
    Ok([
        time_low[0],
        time_low[1],
        time_low[2],
        time_low[3],
        time_mid[0],
        time_mid[1],
        time_high_and_version[0],
        time_high_and_version[1],
        (((s & 0x3F00) >> 8) as u8) | 0x80,
        (s & 0xFF) as u8,
        w[0],
        w[1],
        machine_id[0],
        machine_id[1],
        machine_id[2],
        machine_id[3],
    ])
}

///
/// for network
///
/// 42 bit timestamp 819_200 ns * 2 ^ 42 (114 years)
///
/// 14 bit sequence
///
/// 16 bit worker_id (max threads number 65_536)
///
/// 24 bit machine_id (max machines number 16_777_216 )
///
/// Big Endian Order
///
/// epoch: 100 nanosecond timestamp , unix epoch
///
/// Max IDs per Second : 20_000_000
///
pub fn next_short_96(machine_id: [u8; 3], epoch: u64) -> Result<[u8; 12], Error> {
    let (mut t, s) = next(1 << TIMESTAMP42SHIFT)?;
    t = (t
        .checked_sub(UUID_TICKS_BETWEEN_EPOCHS)
        .ok_or_else(|| Error::EpochException)?
        .checked_sub(epoch)
        .ok_or_else(|| Error::EpochException)?)
        >> TIMESTAMP42SHIFT;
    let t_hi = (t >> 2).to_be_bytes();
    let [t_low_and_s_hi, s_low] = (((t as u16) << 14) | s).to_be_bytes();
    let w = worker_id();
    Ok([
        t_hi[3],
        t_hi[4],
        t_hi[5],
        t_hi[6],
        t_hi[7],
        t_low_and_s_hi,
        s_low,
        w[0],
        w[1],
        machine_id[0],
        machine_id[1],
        machine_id[2],
    ])
}

pub fn short_96_to_128(short_96: [u8; 12], epoch: u64, machine_id_hi: u8) -> [u8; 16] {
    let c = short_96;
    let t = ((u64::from_le_bytes([c[5], c[4], c[3], c[2], c[1], c[0], 0, 0]) >> 6)
        << TIMESTAMP42SHIFT)
        + epoch
        + UUID_TICKS_BETWEEN_EPOCHS;
    let s = u16::from_le_bytes([c[6], c[5]]) & 0x3fff;
    let time_low = ((t & 0xFFFF_FFFF) as u32).to_be_bytes();
    let time_mid = (((t >> 32) & 0xFFFF) as u16).to_be_bytes();
    let time_high_and_version = ((((t >> 48) & 0x0FFF) as u16) | (1 << 12)).to_be_bytes();
    [
        time_low[0],
        time_low[1],
        time_low[2],
        time_low[3],
        time_mid[0],
        time_mid[1],
        time_high_and_version[0],
        time_high_and_version[1],
        (((s & 0x3F00) >> 8) as u8) | 0x80,
        (s & 0xFF) as u8,
        c[7],
        c[8],
        machine_id_hi,
        c[9],
        c[10],
        c[11],
    ]
}

///
/// for standalone
///
/// 42 bit timestamp 819_200 ns * 2 ^ 42 (114 years)
///
/// 14 bit sequence
///
/// 8 bit worker_id (max threads number 256)
///
/// epoch: 100 nanosecond timestamp , unix epoch
///
/// Max IDs per Second : 20_000_000
///
pub fn next_short_64(epoch: u64) -> Result<[u8; 8], Error> {
    let w = worker_id();
    if w[0] != 0 {
        return Err(Error::WorkerIDOverflow);
    }
    let (mut t, s) = next(10000)?;
    t = (t
        .checked_sub(UUID_TICKS_BETWEEN_EPOCHS)
        .ok_or_else(|| Error::EpochException)?
        .checked_sub(epoch)
        .ok_or_else(|| Error::EpochException)?)
        >> TIMESTAMP42SHIFT;
    let t_hi = (t >> 2).to_be_bytes();
    let [t_low_and_s_hi, s_low] = (((t as u16) << 14) | s).to_be_bytes();
    Ok([
        t_hi[3],
        t_hi[4],
        t_hi[5],
        t_hi[6],
        t_hi[7],
        t_low_and_s_hi,
        s_low,
        w[1],
    ])
}

pub fn short_64_to_96(short_64: [u8; 8], machine_id: [u8; 3]) -> [u8; 12] {
    let s = short_64;
    [
        s[0],
        s[1],
        s[2],
        s[3],
        s[4],
        s[5],
        s[6],
        0,
        s[7],
        machine_id[0],
        machine_id[1],
        machine_id[2],
    ]
}

pub fn short_64_to_128(short_64: [u8; 8], epoch: u64, machine_id: [u8; 4]) -> [u8; 16] {
    let short96 = short_64_to_96(short_64, [machine_id[1], machine_id[2], machine_id[3]]);
    short_96_to_128(short96, epoch, machine_id[0])
}

#[test]
fn test_128() {
    use uuid::Uuid;
    use uuid::Variant;
    let id = next_short_128([1, 1, 1, 1]).unwrap();
    let hex: String = id.into_iter().map(|val| format!("{:0>2x}", val)).collect();
    let my_uuid = Uuid::parse_str(&hex).unwrap();
    let (ticks, counter) = my_uuid.to_timestamp().unwrap().to_rfc4122();
    assert_eq!(ticks, timestamp());
    assert_eq!(counter, seq());
    assert_eq!(my_uuid.get_version_num(), 1usize);
    assert_eq!(my_uuid.get_variant().unwrap(), Variant::RFC4122);
}

#[test]
fn test_96() {
    use uuid::Uuid;
    use uuid::Variant;
    let id96 = next_short_96([1, 1, 1], 0).unwrap();
    let id128 = short_96_to_128(id96, 0, 0);
    let hex: String = id128
        .into_iter()
        .map(|val| format!("{:0>2x}", val))
        .collect();
    let my_uuid = Uuid::parse_str(&hex).unwrap();
    let (ticks, counter) = my_uuid.to_timestamp().unwrap().to_rfc4122();
    assert_eq!(ticks, timestamp() >> TIMESTAMP42SHIFT << TIMESTAMP42SHIFT);
    assert_eq!(counter, seq());
    assert_eq!(my_uuid.get_version_num(), 1usize);
    assert_eq!(my_uuid.get_variant().unwrap(), Variant::RFC4122);
}

#[test]
fn test_64() {
    use uuid::Uuid;
    use uuid::Variant;
    let id64 = next_short_64(0).unwrap();
    let id128 = short_64_to_128(id64, 0, [1u8, 1, 1, 1]);
    let hex: String = id128
        .into_iter()
        .map(|val| format!("{:0>2x}", val))
        .collect();
    let my_uuid = Uuid::parse_str(&hex).unwrap();
    let (ticks, counter) = my_uuid.to_timestamp().unwrap().to_rfc4122();
    assert_eq!(ticks, timestamp() >> TIMESTAMP42SHIFT << TIMESTAMP42SHIFT);
    assert_eq!(counter, seq());
    assert_eq!(my_uuid.get_version_num(), 1usize);
    assert_eq!(my_uuid.get_variant().unwrap(), Variant::RFC4122);
}

use std::sync::atomic::{AtomicU16, AtomicU64};

static mut TIMESTAMP_ATOM: AtomicU64 = AtomicU64::new(0);
static mut SEQ_ATOM: AtomicU16 = AtomicU16::new(0);

fn next_atom() -> Result<(u64, u16), Error> {
    unsafe {
        let seq = SEQ_ATOM.get_mut();
        let ts = TIMESTAMP_ATOM.get_mut();
        if *ts == 0 {
            *ts = now()?;
        }
        if *seq < ((1 << 14) - 1) {
            *seq += 1;
        } else {
            if *ts >= now()? {
                return Err(Error::TimeOverflow);
            }
            *ts += 1;
            *seq = 0;
        };
        Ok((*ts, *seq))
    }
}

///
/// uuidv1 generator
///
pub fn uuidv1(machine_id: [u8; 6]) -> Result<[u8; 16], Error> {
    let (t, s) = next_atom()?;
    let time_low = ((t & 0xFFFF_FFFF) as u32).to_be_bytes();
    let time_mid = (((t >> 32) & 0xFFFF) as u16).to_be_bytes();
    let time_high_and_version = ((((t >> 48) & 0x0FFF) as u16) | (1 << 12)).to_be_bytes();
    Ok([
        time_low[0],
        time_low[1],
        time_low[2],
        time_low[3],
        time_mid[0],
        time_mid[1],
        time_high_and_version[0],
        time_high_and_version[1],
        (((s & 0x3F00) >> 8) as u8) | 0x80,
        (s & 0xFF) as u8,
        machine_id[0],
        machine_id[1],
        machine_id[2],
        machine_id[3],
        machine_id[4],
        machine_id[5],
    ])
}

///
/// uuidv1 generator
///
pub fn next_short_128_sync(machine_id: [u8; 6]) -> Result<[u8; 16], Error> {
    uuidv1(machine_id)
}

#[cfg(test)]
fn timestamp_sync() -> u64 {
    unsafe { *TIMESTAMP_ATOM.get_mut() }
}

#[cfg(test)]
fn seq_sync() -> u16 {
    unsafe { *SEQ_ATOM.get_mut() }
}

#[test]
fn test_uuidv1() {
    use uuid::Uuid;
    use uuid::Variant;
    let id128 = next_short_128_sync([0, 0, 0, 0, 0, 0]).unwrap();
    let hex: String = id128
        .into_iter()
        .map(|val| format!("{:0>2x}", val))
        .collect();
    let my_uuid = Uuid::parse_str(&hex).unwrap();
    let (ticks, counter) = my_uuid.to_timestamp().unwrap().to_rfc4122();
    assert_eq!(ticks, timestamp_sync());
    assert_eq!(counter, seq_sync());
    assert_eq!(my_uuid.get_version_num(), 1usize);
    assert_eq!(my_uuid.get_variant().unwrap(), Variant::RFC4122);
}
