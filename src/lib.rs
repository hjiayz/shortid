use ::failure::Fail;

#[derive(Fail, Debug)]
pub enum Error {
    #[fail(display = "There is not enough ID now")]
    TimeOverflow,
    #[fail(display = "SystemTime before UNIX EPOCH!")]
    SystemTimeException,
    #[fail(display = "too many threads")]
    WorkerIDOverflow,
}

use std::sync::atomic::{AtomicUsize, Ordering};

pub static mut COUNTER: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static WORKER_ID: usize = {
        unsafe{
            COUNTER.fetch_add(1, Ordering::SeqCst)
        }
    };
}

fn now() -> Result<u64, Error> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Error::SystemTimeException)?;
    Ok(time.as_secs() as u64 * 1_000 as u64 + time.subsec_millis() as u64)
}

fn worker_id() -> usize {
    WORKER_ID.with(|f| f.to_owned())
}

//for compatible UUID

pub struct ShortID128 {
    machine_id: u32,
    worker_id: u16,
    timestamp: u64,
    sequence: u32,
}

impl ShortID128 {
    pub fn new(machine_id: u32) -> Result<ShortID128, Error> {
        let worker_id = worker_id();
        if worker_id > (u16::max_value() as usize) {
            return Err(Error::WorkerIDOverflow);
        };
        Ok(ShortID128 {
            machine_id: machine_id,
            worker_id: worker_id as u16,
            timestamp: now()?,
            sequence: 0,
        })
    }

    pub fn next(&mut self) -> Result<[u8; 16], Error> {
        if self.sequence < u32::max_value() {
            self.sequence += 1;
            return Ok(self.to_be_bytes());
        };
        if (self.timestamp + 1) < now()? {
            self.timestamp += 1;
            self.sequence = 0;
            return Ok(self.to_be_bytes());
        };
        return Err(Error::TimeOverflow);
    }

    pub fn as_u128(&self) -> u128 {
        u128::from_be_bytes(self.to_be_bytes())
    }

    pub fn to_be_bytes(&self) -> [u8; 16] {
        let t = self.timestamp.to_be_bytes();
        let s = self.sequence.to_be_bytes();
        let m = self.machine_id.to_be_bytes();
        let w = self.worker_id.to_be_bytes();
        [
            t[2], t[3], t[4], t[5], t[6], t[7], s[0], s[1], s[2], s[3], w[0], w[1], m[0], m[1],
            m[2], m[3],
        ]
    }

    pub fn from_be_bytes(b: &[u8; 16]) -> ShortID128 {
        let t = u64::from_be_bytes([0, 0, b[0], b[1], b[2], b[3], b[4], b[5]]);
        let s = u32::from_be_bytes([b[6], b[7], b[8], b[9]]);
        let w = u16::from_be_bytes([b[10], b[11]]);
        let m = u32::from_be_bytes([b[12], b[13], b[14], b[15]]);
        ShortID128 {
            timestamp: t,
            sequence: s,
            machine_id: m,
            worker_id: w,
        }
    }
}

pub struct ShortID96 {
    epoch: u64,
    machine_id: u32,
    worker_id: u8,
    timestamp: u64,
    sequence: u16,
}

impl ShortID96 {
    pub fn new(epoch: u64, machine_id: u32) -> Result<ShortID96, Error> {
        let worker_id = worker_id();
        if worker_id > (u8::max_value() as usize) {
            return Err(Error::WorkerIDOverflow);
        };
        Ok(ShortID96 {
            epoch,
            machine_id,
            worker_id: worker_id as u8,
            timestamp: now()?,
            sequence: 0,
        })
    }

    pub fn next(&mut self) -> Result<[u8; 12], Error> {
        if self.sequence < u16::max_value() {
            self.sequence += 1;
            return Ok(self.to_be_bytes());
        };
        if (self.timestamp + 1) < now()? {
            self.timestamp += 1;
            self.sequence = 0;
            return Ok(self.to_be_bytes());
        };
        return Err(Error::TimeOverflow);
    }

    pub fn as_u128(&self) -> u128 {
        let d = self.to_be_bytes();
        u128::from_be_bytes([
            0, 0, 0, 0, d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7], d[8], d[9], d[10], d[11],
        ])
    }

    pub fn to_be_bytes(&self) -> [u8; 12] {
        let t = (self.timestamp - self.epoch).to_be_bytes();
        let s = self.sequence.to_be_bytes();
        let m = self.machine_id.to_be_bytes();
        let w = self.worker_id;
        [
            t[3], t[4], t[5], t[6], t[7], s[0], s[1], w, m[0], m[1], m[2], m[3],
        ]
    }

    pub fn from_be_bytes(epoch: u64, b: &[u8; 16]) -> ShortID96 {
        let t = u64::from_be_bytes([0, 0, 0, b[0], b[1], b[2], b[3], b[4]]);
        let s = u16::from_be_bytes([b[5], b[6]]);
        let w = b[7];
        let m = u32::from_be_bytes([b[8], b[9], b[10], b[11]]);
        ShortID96 {
            epoch: epoch,
            timestamp: t.saturating_add(epoch),
            sequence: s,
            worker_id: w,
            machine_id: m,
        }
    }

    pub fn upgrade128(&self) -> ShortID128 {
        ShortID128 {
            timestamp: self.timestamp,
            sequence: self.sequence as u32,
            machine_id: self.machine_id,
            worker_id: self.worker_id as u16,
        }
    }

    pub fn to_be_bytes128(&self) -> [u8; 16] {
        self.upgrade128().to_be_bytes()
    }
}

//for standalone

pub struct ShortID64 {
    epoch: u64,
    worker_id: u8,
    timestamp: u64,
    sequence: u16,
}

impl ShortID64 {
    pub fn new(epoch: u64) -> Result<ShortID64, Error> {
        let worker_id = worker_id();
        if worker_id > (u8::max_value() as usize) {
            return Err(Error::WorkerIDOverflow);
        };
        Ok(ShortID64 {
            epoch: epoch,
            worker_id: worker_id as u8,
            timestamp: now()?,
            sequence: 0,
        })
    }

    pub fn next(&mut self) -> Result<[u8; 8], Error> {
        if self.sequence < u16::max_value() {
            self.sequence += 1;
            return Ok(self.to_be_bytes());
        };
        if (self.timestamp + 1) < now()? {
            self.timestamp += 1;
            self.sequence = 0;
            return Ok(self.to_be_bytes());
        };
        return Err(Error::TimeOverflow);
    }

    pub fn as_u64(&self) -> u64 {
        u64::from_be_bytes(self.to_be_bytes())
    }

    pub fn to_be_bytes(&self) -> [u8; 8] {
        let t = (self.timestamp - self.epoch).to_be_bytes();
        let s = self.sequence.to_be_bytes();
        let w = self.worker_id;
        [t[3], t[4], t[5], t[6], t[7], s[0], s[1], w]
    }

    pub fn from_be_bytes(epoch: u64, b: &[u8; 16]) -> ShortID64 {
        let t = u64::from_be_bytes([0, 0, 0, b[0], b[1], b[2], b[3], b[4]]);
        let s = u16::from_be_bytes([b[5], b[6]]);
        let w = b[7];
        ShortID64 {
            epoch: epoch,
            timestamp: t.saturating_add(epoch),
            sequence: s,
            worker_id: w,
        }
    }

    pub fn upgrade96(&self, machine_id: u32) -> ShortID96 {
        ShortID96 {
            epoch: self.epoch,
            timestamp: self.timestamp,
            sequence: self.sequence,
            machine_id,
            worker_id: self.worker_id,
        }
    }

    pub fn to_be_bytes96(&self, machine_id: u32) -> [u8; 12] {
        self.upgrade96(machine_id).to_be_bytes()
    }

    pub fn upgrade128(&self, machine_id: u32) -> ShortID128 {
        ShortID128 {
            timestamp: self.timestamp,
            sequence: self.sequence as u32,
            machine_id,
            worker_id: self.worker_id as u16,
        }
    }

    pub fn to_be_bytes128(&self, machine_id: u32) -> [u8; 16] {
        self.upgrade128(machine_id).to_be_bytes()
    }
}

pub struct Builder128 {
    machine_id: u32,
}

impl Builder128 {
    pub fn new(machine_id: u32) -> Builder128 {
        Builder128 { machine_id }
    }
    pub fn build(&self) -> Result<ShortID128, Error> {
        ShortID128::new(self.machine_id)
    }
}

pub struct Builder96 {
    machine_id: u32,
    epoch: u64,
}

impl Builder96 {
    pub fn new(epoch: u64, machine_id: u32) -> Builder96 {
        Builder96 { machine_id, epoch }
    }
    pub fn build(&self) -> Result<ShortID96, Error> {
        ShortID96::new(self.epoch, self.machine_id)
    }
}

pub struct Builder64 {
    epoch: u64,
}

impl Builder64 {
    pub fn new(epoch: u64) -> Builder64 {
        Builder64 { epoch }
    }
    pub fn build(&self) -> Result<ShortID64, Error> {
        ShortID64::new(self.epoch)
    }
}
