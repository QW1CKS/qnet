fn main() {
    // Only run tauri_build when the crate feature `with-tauri` is enabled.
    // Cargo exposes features to build.rs via env vars like CARGO_FEATURE_WITH_TAURI.
    let with_tauri = std::env::var_os("CARGO_FEATURE_WITH_TAURI").is_some();

    if with_tauri {
        // Ensure a minimal Windows icon exists to satisfy tauri-build on Windows.
        #[cfg(target_os = "windows")]
        {
            use std::fs;
            use std::io::Write;
            use std::path::Path;

            let icon_dir = Path::new("icons");
            let icon_path = icon_dir.join("icon.ico");
            if !icon_path.exists() {
                let _ = fs::create_dir_all(icon_dir);
                // Generate a tiny 1x1 32bpp ICO with opaque white pixel.
                // ICO Header
                let mut data: Vec<u8> = vec![
                    0x00, 0x00, // reserved
                    0x01, 0x00, // type = icon
                    0x01, 0x00, // count = 1
                    // Directory entry (16 bytes)
                    0x01, // width = 1
                    0x01, // height = 1
                    0x00, // color count
                    0x00, // reserved
                    0x01, 0x00, // planes
                    0x20, 0x00, // bitcount = 32
                    0x30, 0x00, 0x00, 0x00, // bytes in image = 48
                    0x16, 0x00, 0x00, 0x00, // offset to image data = 22
                ];
                // BITMAPINFOHEADER (40 bytes)
                let mut bih: Vec<u8> = vec![0; 40];
                // biSize
                bih[0..4].copy_from_slice(&40u32.to_le_bytes());
                // biWidth
                bih[4..8].copy_from_slice(&1u32.to_le_bytes());
                // biHeight (XOR+AND, so double)
                bih[8..12].copy_from_slice(&2u32.to_le_bytes());
                // biPlanes
                bih[12..14].copy_from_slice(&1u16.to_le_bytes());
                // biBitCount
                bih[14..16].copy_from_slice(&32u16.to_le_bytes());
                // biCompression = BI_RGB
                // biSizeImage = 4 (XOR bitmap only; AND mask is separate)
                bih[20..24].copy_from_slice(&4u32.to_le_bytes());
                // rest remain zero
                data.extend_from_slice(&bih);
                // XOR bitmap (1 pixel, BGRA) = white, opaque
                data.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]);
                // AND mask: 1 row padded to 32 bits => 4 bytes, all zeros (opaque)
                data.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);

                let mut f = fs::File::create(&icon_path).expect("create icon.ico");
                f.write_all(&data).expect("write icon.ico");
            }
        }

        // Generate Tauri context from tauri.conf.json and embed platform resources
        tauri_build::build();
    } else {
        // Headless build: emit a hint for Cargo and avoid tauri setup.
        println!("cargo:rerun-if-env-changed=CARGO_FEATURE_WITH_TAURI");
        println!("cargo:warning=stealth-browser building without Tauri (feature `with-tauri` not enabled)");
    }
}
