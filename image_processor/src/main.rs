use std::ffi::CString;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use anyhow::Context;
use clap::{Parser, ValueEnum};
use libloading::{Library, Symbol};
use std::env::consts::{DLL_PREFIX, DLL_EXTENSION};
use std::os::raw::c_char;

type ProcessImageFn = unsafe extern "C" fn(u32, u32, *mut u8, *const c_char);

#[derive(Parser)]
#[command(name = "image_processor", about = "CLI-утилита для обработки изображений", version)]
struct Args {
    #[arg(short, long)]
    input: PathBuf,

    #[arg(short, long)]
    output: PathBuf,

    #[arg(short, long)]
    params: PathBuf,

    #[arg(short, long)]
    plugin: Plugin,
    
    #[arg(short, long, default_value = "./target/debug")]
    plugin_path: PathBuf
}

#[derive(Clone, ValueEnum)]
enum Plugin {
    Blur,
    Mirror
}

impl std::fmt::Display for Plugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Plugin::Blur => write!(f, "blur"),
            Plugin::Mirror => write!(f, "mirror")
        }
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let mut buf = String::new();
    File::open(args.params)?.read_to_string(&mut buf)?;
    let params = CString::new(buf)?;

    let mut image = image::open(&args.input)
        .context("failed to open input image file")?
        .into_rgba8();

    let plugin_path = &args.plugin_path.join(format!("{}{}.{}", DLL_PREFIX, &args.plugin, DLL_EXTENSION));

    let lib = unsafe { Library::new(plugin_path) }.context("failed to load plugin")?;
    let process: Symbol<ProcessImageFn> = unsafe { lib.get(b"process_image\0") }?;

    unsafe { process(image.width(), image.height(), image.as_mut_ptr(), params.as_ptr()) };

    image.save_with_format(&args.output, image::ImageFormat::Png)?;

    Ok(())
}
