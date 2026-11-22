use std::io;

use io::BufWriter;
use io::Write;

use io::BufRead;
use io::Read;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct SeekTableNoSum {
    pub compressed_size: u32,
    pub original_size: u32,
}

pub fn line2compressed(
    original: &[u8],
    buf: &mut Vec<u8>,
    level: Level,
) -> Result<SeekTableNoSum, io::Error> {
    let osz: usize = original.len();
    let original_size: u32 = osz.try_into().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid original size: {osz}"),
        )
    })?;

    let target_size: usize = zrip::compress_bound(osz);
    let buf_size: usize = buf.len();
    let too_small: bool = buf_size < target_size;
    if too_small {
        buf.resize(target_size, 0);
    }

    let out: &mut [u8] = &mut buf[..];
    let csz: usize = zrip::compress_into(original, out, level.into()).map_err(io::Error::other)?;
    let compressed_size: u32 = csz.try_into().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid compressed size: {csz}"),
        )
    })?;

    Ok(SeekTableNoSum {
        compressed_size,
        original_size,
    })
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Level {
    level: i32,
}

impl Level {
    pub const FASTEST: Self = Self { level: -7 };

    pub const FAST6: Self = Self { level: -6 };
    pub const FAST5: Self = Self { level: -5 };
    pub const FAST4: Self = Self { level: -4 };
    pub const FAST3: Self = Self { level: -3 };
    pub const FAST2: Self = Self { level: -2 };
    pub const FAST1: Self = Self { level: -1 };

    pub const DEFAULT: Self = Self { level: 0 };

    pub const GOOD1: Self = Self { level: 1 };
    pub const GOOD2: Self = Self { level: 2 };

    pub const BETTER3: Self = Self { level: 3 };
    pub const BETTER4: Self = Self { level: 4 };
}

impl std::str::FromStr for Level {
    type Err = io::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "-7" | "fast" => Ok(Self::FASTEST),
            "-6" | "fast6" => Ok(Self::FAST6),
            "-5" | "fast5" => Ok(Self::FAST5),
            "-4" | "fast4" => Ok(Self::FAST4),
            "-3" | "fast3" => Ok(Self::FAST3),
            "-2" | "fast2" => Ok(Self::FAST2),
            "-1" | "fast1" => Ok(Self::FAST1),
            "" => Ok(Self::DEFAULT),
            "1" | "good1" => Ok(Self::GOOD1),
            "2" | "good2" => Ok(Self::GOOD2),
            "3" | "best3" => Ok(Self::BETTER3),
            "4" | "best4" => Ok(Self::BETTER4),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("invalid level: {s}"),
            )),
        }
    }
}

impl Default for Level {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl Level {
    pub fn raw_level(&self) -> i32 {
        self.level
    }
}

impl From<Level> for i32 {
    fn from(l: Level) -> Self {
        l.raw_level()
    }
}

pub fn brdr2compressed<R>(
    lines: &mut R,
    buf4orig: &mut Vec<u8>,
    buf4zstd: &mut Vec<u8>,
    level: Level,
    max_line: u32,
) -> Result<Option<SeekTableNoSum>, io::Error>
where
    R: BufRead,
{
    buf4orig.clear();

    let mut taken = Read::take(lines, max_line.into());
    let read_bytes: usize = taken.read_until(b'\n', buf4orig)?;
    if 0 == read_bytes {
        // EOF
        return Ok(None);
    }

    let line: &[u8] = buf4orig;
    line2compressed(line, buf4zstd, level).map(Some)
}

pub fn lines2compressed2writer<R, W>(
    lines: &mut R,
    buf4orig: &mut Vec<u8>,
    buf4zstd: &mut Vec<u8>,
    buf4frame: &mut Vec<SeekTableNoSum>,
    wtr: &mut W,
    cfg: &Config,
) -> Result<(), io::Error>
where
    R: BufRead,
    W: Write,
{
    buf4frame.clear();

    let max_frames: u32 = cfg.max_frame_count;
    let maxu: usize = cfg.max_frame_count.try_into().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid max frames: {max_frames}"),
        )
    })?;
    loop {
        let sz: usize = buf4frame.len();
        let too_many: bool = maxu < sz;
        if too_many {
            return Err(io::Error::new(
                io::ErrorKind::QuotaExceeded,
                format!("too many frames already({maxu} < {sz})"),
            ));
        }

        let oseek: Option<SeekTableNoSum> =
            brdr2compressed(lines, buf4orig, buf4zstd, cfg.level, cfg.max_line_len)?;

        let Some(seek) = oseek else {
            // EOF
            return Ok(());
        };

        buf4frame.push(seek);

        let zsz: usize = seek.compressed_size.try_into().map_err(io::Error::other)?;
        let z: &[u8] = &buf4zstd[..zsz];

        wtr.write_all(z)?;
    }
}

pub fn brdr2compressed2wtr<R, W>(
    mut brdr: R,
    buf4orig: &mut Vec<u8>,
    buf4zstd: &mut Vec<u8>,
    buf4frame: &mut Vec<SeekTableNoSum>,
    mut wtr: W,
    cfg: &Config,
) -> Result<(), io::Error>
where
    R: BufRead,
    W: Write,
{
    lines2compressed2writer(&mut brdr, buf4orig, buf4zstd, buf4frame, &mut wtr, cfg)?;

    let number_of_frames: u32 = buf4frame.len().try_into().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("too many frames: {}", buf4frame.len()),
        )
    })?;

    let table_size_tot: u32 = number_of_frames
        .checked_mul(4 + 4)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "too big table"))?;

    let skippable_size: u32 = table_size_tot
        .checked_add(4 + 1 + 4)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "too big skippable data"))?;

    wtr.write_all(&[0x5e, 0x2a, 0x4d, 0x18])?;
    let ssle: [u8; 4] = skippable_size.to_le_bytes();
    wtr.write_all(&ssle)?;

    for tab in buf4frame {
        let csz: u32 = tab.compressed_size;
        let osz: u32 = tab.original_size;

        let cls: [u8; 4] = csz.to_le_bytes();
        let ols: [u8; 4] = osz.to_le_bytes();

        wtr.write_all(&cls)?;
        wtr.write_all(&ols)?;
    }

    let lnof: [u8; 4] = number_of_frames.to_le_bytes();
    wtr.write_all(&lnof)?;
    wtr.write_all(&[0b0000_0000])?;
    wtr.write_all(&[0xb1, 0xea, 0x92, 0x8f])?;

    wtr.flush()
}

pub struct Config {
    pub bufcap4orig: usize,
    pub bufcap4zstd: usize,
    pub bufcap4stab: usize,
    pub level: Level,
    pub max_line_len: u32,
    pub max_frame_count: u32,
}

pub const MAX_LINE_LEN_DEFAULT: u32 = 65535;
pub const MAX_FRAME_COUNT_DEFAULT: u32 = 65535;

impl Default for Config {
    fn default() -> Self {
        Self {
            bufcap4orig: 0,
            bufcap4zstd: 0,
            bufcap4stab: 0,
            level: Level::default(),
            max_line_len: MAX_LINE_LEN_DEFAULT,
            max_frame_count: MAX_FRAME_COUNT_DEFAULT,
        }
    }
}

pub fn stdin2compressed2stdout(cfg: &Config) -> Result<(), io::Error> {
    let o = io::stdout();
    let mut ol = o.lock();

    let mut buf4orig: Vec<u8> = Vec::with_capacity(cfg.bufcap4orig);
    let mut buf4zstd: Vec<u8> = Vec::with_capacity(cfg.bufcap4zstd);
    let mut buf4frame: Vec<SeekTableNoSum> = Vec::with_capacity(cfg.bufcap4stab);

    brdr2compressed2wtr(
        io::stdin().lock(),
        &mut buf4orig,
        &mut buf4zstd,
        &mut buf4frame,
        BufWriter::new(&mut ol),
        cfg,
    )?;
    ol.flush()
}

pub fn stdin2compressed2stdout_default() -> Result<(), io::Error> {
    let cfg: Config = Config::default();
    stdin2compressed2stdout(&cfg)
}
