use std::io;
use std::process::ExitCode;

use rs_lines2zstd_seekable::Config;
use rs_lines2zstd_seekable::Level;

use rs_lines2zstd_seekable::MAX_FRAME_COUNT_DEFAULT;
use rs_lines2zstd_seekable::MAX_LINE_LEN_DEFAULT;

use rs_lines2zstd_seekable::stdin2compressed2stdout;

fn io_envkey2val(key: &'static str) -> impl FnOnce() -> Result<String, io::Error> {
    move || std::env::var(key).map_err(io::Error::other)
}

fn string2usize(s: String) -> Result<usize, io::Error> {
    str::parse(&s).map_err(io::Error::other)
}

fn io_bind<F, G, T, U, E>(iofn: F, pure: G) -> impl FnOnce() -> Result<U, E>
where
    F: FnOnce() -> Result<T, E>,
    G: Fn(T) -> Result<U, E>,
{
    move || {
        let t: T = iofn()?;
        pure(t)
    }
}

fn io_bufcap4orig() -> impl FnOnce() -> Result<usize, io::Error> {
    io_bind(io_envkey2val("ENV_BUFCAP4ORIGINAL"), string2usize)
}

fn io_bufcap4zstd() -> impl FnOnce() -> Result<usize, io::Error> {
    io_bind(io_envkey2val("ENV_BUFCAP4ZSTD"), string2usize)
}

fn io_bufcap4stab() -> impl FnOnce() -> Result<usize, io::Error> {
    io_bind(io_envkey2val("ENV_BUFCAP4TABLE"), string2usize)
}

fn string2u32(s: String) -> Result<u32, io::Error> {
    str::parse(&s).map_err(io::Error::other)
}

fn string2level(s: String) -> Result<Level, io::Error> {
    str::parse(&s)
}

fn io_level() -> impl FnOnce() -> Result<Level, io::Error> {
    io_bind(io_envkey2val("ENV_LEVEL"), string2level)
}

fn io_max_line() -> impl FnOnce() -> Result<u32, io::Error> {
    io_bind(io_envkey2val("ENV_MAX_LINE_SIZE"), string2u32)
}

fn io_max_frame_count() -> impl FnOnce() -> Result<u32, io::Error> {
    io_bind(io_envkey2val("ENV_MAX_FRAME_COUNT"), string2u32)
}

fn io_config() -> impl FnOnce() -> Config {
    || {
        let bufcap4orig: usize = io_bufcap4orig()().unwrap_or_default();
        let bufcap4zstd: usize = io_bufcap4zstd()().unwrap_or_default();
        let bufcap4stab: usize = io_bufcap4stab()().unwrap_or_default();

        let level: Level = io_level()().unwrap_or_default();

        let max_line_len: u32 = io_max_line()().unwrap_or(MAX_LINE_LEN_DEFAULT);
        let max_frame_count: u32 = io_max_frame_count()().unwrap_or(MAX_FRAME_COUNT_DEFAULT);

        Config {
            bufcap4orig,
            bufcap4zstd,
            bufcap4stab,

            level,

            max_line_len,
            max_frame_count,
        }
    }
}

fn io_config2void(cfg: Config) -> impl FnOnce() -> Result<(), io::Error> {
    move || stdin2compressed2stdout(&cfg)
}

fn io_main() -> impl FnOnce() -> Result<(), io::Error> {
    || {
        let cfg: Config = io_config()();
        io_config2void(cfg)()
    }
}

fn sub() -> Result<(), io::Error> {
    io_main()()
}

fn main() -> ExitCode {
    sub().map(|_| ExitCode::SUCCESS).unwrap_or_else(|e| {
        eprintln!("{e}");
        ExitCode::FAILURE
    })
}
