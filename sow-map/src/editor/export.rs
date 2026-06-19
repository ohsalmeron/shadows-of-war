use std::io::Write;

use super::state::*;

impl MapEditorSession {
    pub(crate) fn compile_map_package(&self) -> Result<MapExportArtifacts, String> {
        if !sow_core::maps::map_within_pixel_budget(self.width, self.height) {
            return Err(format!(
                "Map is {}x{} ({} pixels); max is {}.",
                self.width,
                self.height,
                self.width as u64 * self.height as u64,
                sow_core::maps::MAX_MAP_PIXELS
            ));
        }

        let slug = sow_core::maps::map_key(&self.editor_ui.map_name);
        if slug.is_empty() {
            return Err("Map name must contain at least one letter or number.".into());
        }

        let mut pixels = vec![[0u8; 4]; (self.width * self.height) as usize];
        for (i, &byte) in self.terrain.iter().enumerate() {
            let is_land = (byte & 0b10000000) != 0;
            let mag = byte & 0b00011111;
            let mut blue = 106u8;
            if is_land {
                blue = (mag as u16 + 140).min(200) as u8;
            }
            pixels[i] = [0, 0, blue, 255];
        }

        let thumb_pixels = pixels.clone();
        let args = crate::generator::GeneratorArgs {
            width: self.width,
            height: self.height,
            pixels,
            remove_small: true,
        };
        let result = crate::generator::generate_map(args)?;

        let spawns: Vec<sow_core::map_file::MapSpawn> = self
            .editor_ui
            .spawns
            .iter()
            .map(|s| sow_core::map_file::MapSpawn {
                name: s.name.clone(),
                flag: s.flag.clone(),
                x: s.x,
                y: s.y,
            })
            .collect();
        let map_file = sow_core::map_file::MapFile {
            display_name: self.editor_ui.map_name.clone(),
            width: result.width,
            height: result.height,
            num_land_tiles: result.num_land_tiles,
            spawns,
            terrain: result.map_data,
        };
        let map_bytes = sow_core::map_file::encode(&map_file);

        let mut brotli_bytes = Vec::new();
        {
            let mut writer = brotli::CompressorWriter::new(&mut brotli_bytes, 4096, 11, 22);
            writer.write_all(&map_bytes).map_err(|e| e.to_string())?;
            writer.flush().map_err(|e| e.to_string())?;
        }

        use image::{DynamicImage, RgbaImage};
        let rgba = RgbaImage::from_raw(
            self.width,
            self.height,
            thumb_pixels
                .iter()
                .flat_map(|p| p.iter().copied())
                .collect(),
        )
        .ok_or_else(|| "thumbnail pixel buffer size mismatch".to_string())?;
        let thumb_webp =
            crate::thumbnail::encode_square_thumbnail_webp(&DynamicImage::ImageRgba8(rgba))?;

        Ok(MapExportArtifacts {
            slug,
            map_bytes,
            brotli_bytes,
            thumb_webp,
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn write_map_package_native(
        &mut self,
        artifacts: MapExportArtifacts,
        strings: &sow_i18n::MapEditorStrings,
    ) {
        let maps_root = Self::maps_root();
        let out_dir = maps_root.join(&artifacts.slug);
        if let Err(e) = std::fs::create_dir_all(&out_dir) {
            self.notify_error(format!("Failed to create path: {e}"));
            return;
        }

        let wrote_map = std::fs::write(out_dir.join("map.bin"), &artifacts.map_bytes).is_ok();
        let wrote_br = std::fs::write(out_dir.join("map.bin.br"), &artifacts.brotli_bytes).is_ok();
        let wrote_thumb =
            std::fs::write(out_dir.join("thumbnail.webp"), &artifacts.thumb_webp).is_ok();

        if wrote_map && wrote_br && wrote_thumb {
            match Self::refresh_maps_catalog(&maps_root) {
                Ok(()) => {
                    if let Err(e) = Self::reload_local_map_catalog(
                        &mut self.client_app,
                        &self.egui_ctx,
                        Some(&artifacts.slug),
                    ) {
                        self.notify_info(format!("{} (catalog reload: {e})", strings.msg_saved));
                    } else {
                        self.notify_info(&strings.msg_saved_sp);
                    }
                }
                Err(e) => {
                    self.notify_info(format!("{} (catalog: {e})", strings.msg_saved));
                }
            }
        } else {
            self.notify_error(&strings.msg_write_failed);
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn download_map_package_wasm(
        &mut self,
        artifacts: MapExportArtifacts,
        strings: &sow_i18n::MapEditorStrings,
    ) {
        let prefix = &artifacts.slug;
        crate::wasm_export::trigger_download(&format!("{prefix}/map.bin"), &artifacts.map_bytes);
        crate::wasm_export::trigger_download(
            &format!("{prefix}/map.bin.br"),
            &artifacts.brotli_bytes,
        );
        crate::wasm_export::trigger_download(
            &format!("{prefix}/thumbnail.webp"),
            &artifacts.thumb_webp,
        );
        self.notify_info(&strings.msg_saved_download);
    }

    pub(crate) fn export_map_package(&mut self) {
        log::info!("Compiling map package from editor...");
        let lang = self.client_app.settings_state.language;
        let strings = &sow_i18n::get(lang).map_editor;

        self.editor_ui.exporting = true;
        self.editor_ui.busy_message = Some(strings.msg_compiling.clone());
        self.notify_info(&strings.msg_compiling);

        match self.compile_map_package() {
            Ok(artifacts) => {
                #[cfg(not(target_arch = "wasm32"))]
                self.write_map_package_native(artifacts, strings);
                #[cfg(target_arch = "wasm32")]
                self.download_map_package_wasm(artifacts, strings);
            }
            Err(e) => {
                self.notify_error(format!("Compilation error: {e}"));
            }
        }
        self.editor_ui.clear_busy();
    }
}
