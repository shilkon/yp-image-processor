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

    #[arg(long)]
    params: PathBuf,

    #[arg(long)]
    plugin: Plugin,
    
    #[arg(long, default_value = "./target/debug")]
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

    // Safety: 
    // 1. Загрузка плагина: Плагин не задействет собственные функции инициализации
    // 2. Поиск символа: Возвращаемый тип для символа "process_image" указан корректно `ProcessImageFn`
    // 3. Вызов функции: Передаваемый указатель `image.as_mut_ptr()` валиден для записи/чтения
    //    в объеме `width * height * 4` байт, а `params.as_ptr()` является валидной C-строкой.
    // 4. Время жизни: `lib` живет дольше, чем вызываемый из нее функционал.
    unsafe {
        let lib = Library::new(plugin_path).context("failed to load plugin")?;
        let process: Symbol<ProcessImageFn> = lib.get(b"process_image\0")?;

        process(image.width(), image.height(), image.as_mut_ptr(), params.as_ptr());
    }

    image.save_with_format(&args.output, image::ImageFormat::Png)?;

    Ok(())
}
