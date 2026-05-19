use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

const APP_NAME: &str = "Watch";
const APP_EXE_NAME: &str = "Watch.exe";
const ICON_RESOURCE_ID: u16 = 1;
const ICON_IMAGE_SIZE: u32 = 256;
const COLOR_CHANNEL_COUNT: usize = 4;
const ICON_DIRECTORY_SIZE: usize = 6;
const ICON_ENTRY_SIZE: usize = 16;
const BITMAP_INFO_HEADER_SIZE: u32 = 40;
const ICO_PLANE_COUNT: u16 = 1;
const ICO_COLOR_DEPTH: u16 = 32;
const PLAY_MARK_RED: u8 = 255;
const PLAY_MARK_GREEN: u8 = 138;
const PLAY_MARK_BLUE: u8 = 42;
fn main() -> io::Result<()> {
    println!("cargo:rerun-if-changed=src/icons/play.svg");
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=build.rs");

    #[cfg(target_os = "windows")]
    embed_windows_resources()?;

    Ok(())
}

#[cfg(target_os = "windows")]
fn embed_windows_resources() -> io::Result<()> {
    let output_directory = PathBuf::from(
        env::var_os("OUT_DIR")
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "OUT_DIR is not available"))?,
    );
    let play_icon_svg_path = PathBuf::from("src/icons/play.svg");
    let icon_path = output_directory.join("watch-play.ico");
    let resource_path = output_directory.join("watch.rc");

    fs::write(&icon_path, create_play_icon_file(&play_icon_svg_path)?)?;
    fs::write(&resource_path, create_resource_script(&icon_path))?;

    embed_resource::compile(&resource_path, embed_resource::NONE)
        .manifest_optional()
        .map_err(|error| io::Error::new(io::ErrorKind::Other, error.to_string()))?;

    Ok(())
}

#[cfg(target_os = "windows")]
fn create_resource_script(icon_path: &Path) -> String {
    let escaped_icon_path = icon_path.display().to_string().replace('\\', "\\\\");

    format!(
        r#"#include <winver.h>

{ICON_RESOURCE_ID} ICON "{escaped_icon_path}"

1 VERSIONINFO
FILEVERSION 0,1,0,0
PRODUCTVERSION 0,1,0,0
FILEFLAGSMASK 0x3fL
FILEFLAGS 0x0L
FILEOS VOS_NT_WINDOWS32
FILETYPE VFT_APP
FILESUBTYPE 0x0L
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "040904B0"
        BEGIN
            VALUE "CompanyName", "{APP_NAME}\0"
            VALUE "FileDescription", "{APP_NAME}\0"
            VALUE "FileVersion", "0.1.0\0"
            VALUE "InternalName", "{APP_NAME}\0"
            VALUE "OriginalFilename", "{APP_EXE_NAME}\0"
            VALUE "ProductName", "{APP_NAME}\0"
            VALUE "ProductVersion", "0.1.0\0"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x0409, 1200
    END
END
"#
    )
}

#[cfg(target_os = "windows")]
fn create_play_icon_file(play_icon_svg_path: &Path) -> io::Result<Vec<u8>> {
    let play_icon_pixels = render_play_svg_as_bgra_pixels(play_icon_svg_path)?;
    let image_byte_count =
        (ICON_IMAGE_SIZE * ICON_IMAGE_SIZE * COLOR_CHANNEL_COUNT as u32) as usize;
    let bitmap_header_size = BITMAP_INFO_HEADER_SIZE as usize;
    let bitmap_data_size = bitmap_header_size + image_byte_count;
    let bitmap_data_offset = ICON_DIRECTORY_SIZE + ICON_ENTRY_SIZE;
    let mut icon_file = Vec::with_capacity(bitmap_data_offset + bitmap_data_size);

    icon_file.extend_from_slice(&0u16.to_le_bytes());
    icon_file.extend_from_slice(&1u16.to_le_bytes());
    icon_file.extend_from_slice(&1u16.to_le_bytes());
    icon_file.push(0);
    icon_file.push(0);
    icon_file.push(0);
    icon_file.push(0);
    icon_file.extend_from_slice(&ICO_PLANE_COUNT.to_le_bytes());
    icon_file.extend_from_slice(&ICO_COLOR_DEPTH.to_le_bytes());
    icon_file.extend_from_slice(&(bitmap_data_size as u32).to_le_bytes());
    icon_file.extend_from_slice(&(bitmap_data_offset as u32).to_le_bytes());

    icon_file.extend_from_slice(&BITMAP_INFO_HEADER_SIZE.to_le_bytes());
    icon_file.extend_from_slice(&(ICON_IMAGE_SIZE as i32).to_le_bytes());
    icon_file.extend_from_slice(&((ICON_IMAGE_SIZE * 2) as i32).to_le_bytes());
    icon_file.extend_from_slice(&ICO_PLANE_COUNT.to_le_bytes());
    icon_file.extend_from_slice(&ICO_COLOR_DEPTH.to_le_bytes());
    icon_file.extend_from_slice(&0u32.to_le_bytes());
    icon_file.extend_from_slice(&(image_byte_count as u32).to_le_bytes());
    icon_file.extend_from_slice(&0i32.to_le_bytes());
    icon_file.extend_from_slice(&0i32.to_le_bytes());
    icon_file.extend_from_slice(&0u32.to_le_bytes());
    icon_file.extend_from_slice(&0u32.to_le_bytes());

    icon_file.extend_from_slice(&play_icon_pixels);
    Ok(icon_file)
}

#[cfg(target_os = "windows")]
fn render_play_svg_as_bgra_pixels(play_icon_svg_path: &Path) -> io::Result<Vec<u8>> {
    let source_svg = fs::read_to_string(play_icon_svg_path)?;
    let orange_play_svg = source_svg.replace(
        "currentColor",
        &format!("#{PLAY_MARK_RED:02x}{PLAY_MARK_GREEN:02x}{PLAY_MARK_BLUE:02x}"),
    );
    let mut svg_options = resvg::usvg::Options {
        resources_dir: play_icon_svg_path.parent().map(Path::to_path_buf),
        ..Default::default()
    };
    svg_options.fontdb_mut().load_system_fonts();

    let svg_tree = resvg::usvg::Tree::from_data(orange_play_svg.as_bytes(), &svg_options)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
    let svg_size = svg_tree.size();
    let scale_x = ICON_IMAGE_SIZE as f32 / svg_size.width();
    let scale_y = ICON_IMAGE_SIZE as f32 / svg_size.height();
    let mut pixmap = resvg::tiny_skia::Pixmap::new(ICON_IMAGE_SIZE, ICON_IMAGE_SIZE)
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "failed to allocate icon pixmap"))?;

    resvg::render(
        &svg_tree,
        resvg::tiny_skia::Transform::from_scale(scale_x, scale_y),
        &mut pixmap.as_mut(),
    );

    Ok(convert_rgba_pixels_to_bottom_up_bgra(pixmap.data()))
}

#[cfg(target_os = "windows")]
fn convert_rgba_pixels_to_bottom_up_bgra(rgba_pixels: &[u8]) -> Vec<u8> {
    let row_byte_count = (ICON_IMAGE_SIZE * COLOR_CHANNEL_COUNT as u32) as usize;
    let mut bgra_pixels = Vec::with_capacity(rgba_pixels.len());

    for row_index in (0..ICON_IMAGE_SIZE as usize).rev() {
        let row_start = row_index * row_byte_count;
        let row_end = row_start + row_byte_count;
        for pixel in rgba_pixels[row_start..row_end].chunks_exact(COLOR_CHANNEL_COUNT) {
            bgra_pixels.push(pixel[2]);
            bgra_pixels.push(pixel[1]);
            bgra_pixels.push(pixel[0]);
            bgra_pixels.push(pixel[3]);
        }
    }

    bgra_pixels
}
