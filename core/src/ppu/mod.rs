pub mod mode;
pub mod palette;
mod map;
mod sprite;
mod tile;

use mode::{Lcd, LcdResults, LcdModeType};
use map::Map;
use palette::*;
use sprite::{OAM_BYTE_SIZE, Sprite};
use tile::{Tile, TILE_BYTES};
use crate::utils::*;

// =============
// = Constants =
// =============

// VRAM registers
const LCDC: u16                    = 0xFF40;
const STAT: u16                    = 0xFF41;
const SCY: u16                     = 0xFF42;
const SCX: u16                     = 0xFF43;
pub const LY: u16                  = 0xFF44;
const LYC: u16                     = 0xFF45;
// 0xFF46 is DMA transfer, handled by Bus
const BGP: u16                     = 0xFF47;
const OBP0: u16                    = 0xFF48;
const OBP1: u16                    = 0xFF49;
const WY: u16                      = 0xFF4A;
const WX: u16                      = 0xFF4B;
pub const VBK: u16                 = 0xFF4F;

// CGB Palette registers
const BGPI: u16                    = 0xFF68;
const BGPD: u16                    = 0xFF69;
const OBPI: u16                    = 0xFF6A;
const OBPD: u16                    = 0xFF6B;

// VRAM ranges
const TILE_SET: u16                = 0x8000;
const TILE_SET_END: u16            = 0x97FF;
const TILE_MAP: u16                = 0x9800;
const TILE_MAP_END: u16            = 0x9FFF;

const OAM_START: u16               = 0xFE00;
const OAM_END: u16                 = 0xFE9F;
const IO_START: u16                = 0xFF00;
const IO_END: u16                  = 0xFF7F;

// General constants
const MAP_SIZE: usize = 32; // In tiles
const MAP_PIXELS: usize = MAP_SIZE * TILESIZE; // In pixels
const VRAM_BANK_NUM: usize = 2;
const TILE_MAP_SIZE: usize = (TILE_MAP_END - TILE_MAP + 1) as usize;
const TILE_MAP_TBL_SIZE: usize = (TILE_MAP_SIZE / 2) as usize;
const IO_SIZE: usize = (IO_END - IO_START + 1) as usize;
const TILE_NUM: usize = 384;
const OAM_SPR_NUM: usize = 40;
const SPR_PER_LINE: usize = 10;
const CGB_BG_PAL_DATA_SIZE: usize = 64; // 8 palettes, 4 colors per palette, 2 bytes per color
const CGB_SPR_PAL_DATA_SIZE: usize = 64;

// Register bit constants
const BG_DISP_BIT: u8           = 0;
const SPR_DISP_BIT: u8          = 1;
const SPR_SIZE_BIT: u8          = 2;
const BG_TILE_MAP_BIT: u8       = 3;
const BG_WNDW_TILE_DATA_BIT: u8 = 4;
const WNDW_DISP_BIT: u8         = 5;
const WNDW_TILE_MAP_BIT: u8     = 6;
const LCD_DISP_BIT: u8          = 7;

const AUTO_INC_BIT: u8          = 7;

const LYC_LY_FLAG_BIT: u8       = 2;
const HBLANK_INTERRUPT_BIT: u8  = 3;
const VBLANK_INTERRUPT_BIT: u8  = 4;
const OAM_INTERRUPT_BIT: u8     = 5;
const LYC_LY_INTERRUPT_BIT: u8  = 6;

pub struct PpuUpdateResult {
    pub lcd_result: LcdResults,
    pub interrupt: bool,
}

pub struct PPU {
    vram_bank: usize,
    io: [u8; IO_SIZE],
    screen_buffer: [u8; DISP_SIZE],
    tiles: [Tile; VRAM_BANK_NUM * TILE_NUM],
    tile_maps: [Map; VRAM_BANK_NUM * TILE_MAP_SIZE],
    oam: [Sprite; OAM_SPR_NUM],
    last_wndw_line: Option<u8>,
    cgb_bg_pal_data: [u8; CGB_BG_PAL_DATA_SIZE],
    cgb_spr_pal_data: [u8; CGB_SPR_PAL_DATA_SIZE],
    lcd_mode: Lcd,
    palette: Palette,
}

impl Default for PPU {
    fn default() -> Self {
        Self::new()
    }
}

impl PPU {
    // ==================
    // = Public methods =
    // ==================
    pub fn new() -> PPU {
        PPU {
            vram_bank: 0,
            io: [0; IO_SIZE],
            screen_buffer: [0; DISP_SIZE],
            tiles: [Tile::new(); VRAM_BANK_NUM * TILE_NUM],
            tile_maps: [Map::new(); VRAM_BANK_NUM * TILE_MAP_SIZE],
            oam: [Sprite::new(); OAM_SPR_NUM],
            last_wndw_line: None,
            cgb_bg_pal_data: [0; CGB_BG_PAL_DATA_SIZE],
            cgb_spr_pal_data: [0; CGB_SPR_PAL_DATA_SIZE],
            lcd_mode: Lcd::new(),
            palette: Palette::new(),
        }
    }

    /// ```
    /// Write VRAM
    ///
    /// Write value to specified address in VRAM
    ///
    /// Inputs:
    ///     Address to write to (u16)
    ///     Value to write (u8)
    ///     System mode (GB)
    /// ```
    pub fn write_vram(&mut self, addr: u16, val: u8, mode: GB) {
        // TODO: These limitations need to eventually be supported,
        // but due to my poor LCD timer, cause issues due to inaccuracies
        // let lcd_mode = self.lcd_mode.get_mode();

        match addr {
            OAM_START..=OAM_END => {
                // During LCD modes 2 and 3, cannot access OAM
                // if lcd_mode == LcdModeType::OAMReadMode || lcd_mode == LcdModeType::VRAMReadMode {
                //     return;
                // }

                let relative_addr = addr - OAM_START;
                let spr_num = relative_addr / OAM_BYTE_SIZE;
                let byte_num = relative_addr % OAM_BYTE_SIZE;
                self.oam[spr_num as usize].set_byte(byte_num, val, mode);
            },
            TILE_SET..=TILE_SET_END => {
                // During LCD mode 3, cannot access VRAM
                // if lcd_mode == LcdModeType::VRAMReadMode {
                //     return;
                // }

                let offset = addr - TILE_SET;
                let tile_num = (offset / TILE_BYTES) + (self.vram_bank * TILE_NUM) as u16;
                let byte_num = offset % TILE_BYTES;
                self.tiles[tile_num as usize].set_byte(byte_num, val);
            },
            TILE_MAP..=TILE_MAP_END => {
                // During LCD mode 3, cannot access VRAM
                // if lcd_mode == LcdModeType::VRAMReadMode {
                //     return;
                // }

                let map_addr = (addr - TILE_MAP) as usize;
                if self.vram_bank == 0 {
                    self.tile_maps[map_addr].set_tile_num(val);
                } else {
                    assert_ne!(mode, GB::DMG, "VRAM bank can't be greater than 0 for DMG");
                    self.tile_maps[map_addr].set_metadata(val);
                }
            },
            IO_START..=IO_END => {
                match addr {
                    STAT => {
                        let mask = val & 0b1111_1000;
                        let stat = self.read_io(STAT);
                        self.write_io(STAT, mask | stat);
                    },
                    BGPD => {
                        if mode == GB::CGB {
                            // During LCD mode 3, cannot edit palette data
                            // if lcd_mode == LcdModeType::VRAMReadMode {
                            //     return;
                            // }

                            self.write_cgb_bg_color(val);
                        } else {
                            self.write_io(addr, val);
                        }
                    },
                    OBPD => {
                        if mode == GB::CGB {
                            // During LCD mode 3, cannot edit palette data
                            // if lcd_mode == LcdModeType::VRAMReadMode {
                            //     return;
                            // }

                            self.write_cgb_spr_color(val);
                        } else {
                            self.write_io(addr, val);
                        }
                    },
                    VBK => {
                        if mode == GB::CGB {
                            self.set_vram_bank(val);
                        } else {
                            self.write_io(addr, val);
                        }
                    },
                    _ => {
                        self.write_io(addr, val);
                    }
                }
            },
            _ => {}
        }
    }

    /// ```
    /// Read VRAM
    ///
    /// Read value from given address in VRAM
    ///
    /// Input:
    ///     Address to read from (u16)
    ///     Bank override (Option<u16>)
    ///     System mode (GB)
    ///
    /// Output:
    ///     Value at given address (u8)
    /// ```
    pub fn read_vram(&self, addr: u16, bank_override: Option<u16>, mode: GB) -> u8 {
        let bank = if let Some(b) = bank_override {
            b as usize
        } else {
            self.vram_bank
        };

        match addr {
            OAM_START..=OAM_END => {
                let relative_addr = addr - OAM_START;
                let spr_num = relative_addr / OAM_BYTE_SIZE;
                let byte_num = relative_addr % OAM_BYTE_SIZE;
                self.oam[spr_num as usize].get_byte(byte_num)
            },
            TILE_SET..=TILE_SET_END => {
                let offset = addr - TILE_SET;
                let tile_num = (offset / TILE_BYTES) + (bank * TILE_NUM) as u16;
                let byte_num = offset % TILE_BYTES;
                self.tiles[tile_num as usize].get_byte(byte_num)
            },
            TILE_MAP..=TILE_MAP_END => {
                let map_addr = (addr - TILE_MAP) as usize;
                if bank == 0 {
                    self.tile_maps[map_addr].get_tile_num()
                } else {
                    assert_ne!(mode, GB::DMG, "VRAM bank can't be greater than 0 for DMG");
                    self.tile_maps[map_addr].get_metadata()
                }
            },
            IO_START..=IO_END => {
                if mode == GB::CGB {
                    match addr {
                        BGPD => {
                            self.read_cgb_bg_color()
                        },
                        OBPD => {
                            self.read_cgb_spr_color()
                        },
                        VBK => {
                            0xFE + self.vram_bank as u8
                        },
                        _ => {
                            self.read_io(addr)
                        }
                    }
                } else {
                    self.read_io(addr)
                }
            },
            _ => {
                // Unused, do nothing
                0
            }
        }
    }

    pub fn update(&mut self, cycles: u8) -> PpuUpdateResult {
        let old_mode = self.lcd_mode.get_mode();
        let lcd_result = self.lcd_mode.lcd_step(cycles);
        let mut interrupt = self.set_ly();

        // Trigger interrupt if
        // - Mode has changed
        // - Interrupt for that mode is enabled
        let mut stat = self.read_io(STAT);
        let mode = self.lcd_mode.get_mode();
        if old_mode != mode {
            match mode {
                LcdModeType::HBLANK => {
                    interrupt |= stat.get_bit(HBLANK_INTERRUPT_BIT);
                },
                LcdModeType::VBLANK => {
                    interrupt |= stat.get_bit(VBLANK_INTERRUPT_BIT);
                },
                LcdModeType::VRAMReadMode => {
                    interrupt |= stat.get_bit(OAM_INTERRUPT_BIT);
                },
                _ => ()
            }
        }

        // Update the STAT register to match our new LCD mode
        stat &= 0b1111_1100;
        stat |= mode.get_idx();
        self.write_io(STAT, stat);

        PpuUpdateResult{ lcd_result, interrupt }
    }

    pub fn get_lcd_mode(&self) -> LcdModeType {
        self.lcd_mode.get_mode()
    }

    /// ```
    /// Set LY register
    ///
    /// Sets the value at the LY RAM address
    ///
    /// Output:
    ///     Whether values in LY and LYC registers are equal (bool)
    /// ```
    fn set_ly(&mut self) -> bool {
        let line = self.lcd_mode.get_scanline();
        let old_ly = self.read_io(LY);
        if old_ly != line {
            // If we are in a new frame, reset window layer line
            if line == 0 {
                self.last_wndw_line = None;
            }

            self.write_io(LY, line);

            let mut stat = self.read_io(STAT);
            if self.read_io(LY) == self.read_io(LYC) {
                // If LY and LYC are equal:
                // - Set coincidence bit in STAT register
                // - Trigger LCDC status interrupt if enabled
                stat.set_bit(LYC_LY_FLAG_BIT);
                self.write_io(STAT, stat);
                return stat.get_bit(LYC_LY_INTERRUPT_BIT);
            } else {
                stat.clear_bit(LYC_LY_FLAG_BIT);
                self.write_io(STAT, stat);
            }
        }

        false
    }

    /// ```
    /// Render scanline
    ///
    /// Renders specified scanline to buffer
    ///
    /// Input:
    ///     GB hardware type
    /// ```
    pub fn render_scanline(&mut self, mode: GB) {
        // Render current scanline
        let line = self.read_io(LY);
        let mut pixel_row = [0xFF; SCREEN_WIDTH * COLOR_CHANNELS];

        if self.is_bkgd_dspl(mode) {
            self.render_background_line(&mut pixel_row, line, mode);
        }

        if self.is_wndw_dspl() {
            self.render_wndw_line(&mut pixel_row, line, mode);
        }

        if self.is_sprt_dspl() {
            self.render_sprite_line(&mut pixel_row, line, mode);
        }

        // Copy this line of pixels into overall screen buffer
        let start_index = line as usize * (SCREEN_WIDTH * COLOR_CHANNELS);
        let end_index = (line + 1) as usize * (SCREEN_WIDTH * COLOR_CHANNELS);
        self.screen_buffer[start_index..end_index].copy_from_slice(&pixel_row);
    }

    /// ```
    /// Render screen
    ///
    /// Renders the current screen
    ///
    /// Output:
    ///     Array of pixels to draw ([u8])
    /// ```
    pub fn render_screen(&self) -> [u8; DISP_SIZE] {
        let mut map_array = [0xFF; DISP_SIZE];
        if self.is_lcd_dspl() {
            map_array.copy_from_slice(&self.screen_buffer);
        }
        map_array
    }

    /// ```
    /// Set system palette
    ///
    /// Set which color palette we want to use
    ///
    /// Input:
    ///     Palette (Palettes)
    /// ```
    pub fn set_sys_pal(&mut self, pal: Palettes) {
        self.palette.set_sys_pal(pal);
    }

    // ===================
    // = Private methods =
    // ===================

    /// ```
    /// Render Background Line
    ///
    /// Renders the given scanline of the background layer
    ///
    /// Inputs:
    ///     Array to load pixel data into (&[u8])
    ///     Scanline to render (u8)
    ///     Hardware type (GB)
    /// ```
    fn render_background_line(&self, pixel_row: &mut [u8], line: u8, mode: GB) {
        // TODO: This is not ideal. Someday, I'd like to not have this variable if we aren't DMG
        let dmg_pal = self.palette.get_bg_pal();
        let pal_indices = self.get_dmg_bg_indices();
        let screen_coords = self.get_scroll_coords();

        // Get the row of tiles containing our scanline
        let y = ((screen_coords.y as usize) + (line as usize)) % MAP_PIXELS;
        let row = y % TILESIZE;
        let start_x = screen_coords.x as usize;
        for x in 0..SCREEN_WIDTH {
            // Get coords for current tile
            let map_x = ((start_x + x) % MAP_PIXELS) / TILESIZE;
            let map_y = y / TILESIZE;
            // The index is the cell in question, plus the offset for which map table is being used
            let idx = (map_y * MAP_SIZE + map_x) + (self.get_bkgd_tile_map_index() as usize * TILE_MAP_TBL_SIZE);
            let tile_data = self.tile_maps[idx];
            // The tile indexes in the second tile pattern table ($8800-97ff) are signed
            let tile_index = if self.get_bkgd_wndw_tile_set_index() == 0 {
                (256 + (tile_data.get_tile_num() as i8 as isize)) as usize
            } else {
                tile_data.get_tile_num() as usize
            };

            let tile = if mode == GB::CGB {
                let bank_offset = tile_data.get_vram_bank() * TILE_NUM;
                &self.tiles[tile_index + bank_offset]
            } else {
                &self.tiles[tile_index]
            };

            let col = (start_x + x) % TILESIZE;
            let col = if tile_data.is_x_flip() {
                TILESIZE - col - 1
            } else {
                col
            };
            let row = if tile_data.is_y_flip() {
                TILESIZE - row - 1
            } else {
                row
            };

            let pixel = tile.get_row(row)[col] as usize;
            let color = if mode == GB::CGB {
                let pal_indices = self.get_cgb_bg_indices(tile_data.get_pal_num());
                gbc2rgba(pal_indices[2 * pixel], pal_indices[2 * pixel + 1])
            } else {
                dmg_pal[pal_indices[pixel] as usize]
            };

            for i in 0..COLOR_CHANNELS {
                pixel_row[COLOR_CHANNELS * x + i] = color[i];
            }
        }
    }

    /// ```
    /// Render Window Line
    ///
    /// Renders the given scanline of the window layer
    ///
    /// Inputs:
    ///     Array to load pixel data into (&[u8])
    ///     Scanline to render (u8)
    /// ```
    fn render_wndw_line(&mut self, pixel_row: &mut [u8], line: u8, mode: GB) {
        let wndw_coords = self.get_wndw_coords();
        // See below for why this is needed
        let line = if self.last_wndw_line.is_none() { line } else { self.last_wndw_line.unwrap() + 1 };

        // If window isn't drawn on this scanline, return
        if (wndw_coords.y > line) || (wndw_coords.x > SCREEN_WIDTH as u8) {
            return;
        }

        let dmg_pal = self.palette.get_bg_pal();
        let pal_indices = self.get_dmg_bg_indices();

        // Get the row of tiles containing our scanline
        let y = (line - wndw_coords.y) as usize;
        let row = y % TILESIZE;
        let map_y = y / TILESIZE;
        let start_x = wndw_coords.x as usize;
        for x in start_x..SCREEN_WIDTH {
            // Get coords for current tile
            let map_x = ((x - start_x) % MAP_PIXELS) / TILESIZE;
            // The index is the cell in question, plus the offset for which map table is being used
            let idx = (map_y * MAP_SIZE + map_x) + (self.get_wndw_tile_map_index() as usize * TILE_MAP_TBL_SIZE);
            let wndw_data = self.tile_maps[idx];
            // The tile indexes in the second tile pattern table ($8800-97ff) are signed
            let mut tile_index = if self.get_bkgd_wndw_tile_set_index() == 0 {
                (256 + (wndw_data.get_tile_num() as i8 as isize)) as usize
            } else {
                wndw_data.get_tile_num() as usize
            };
            tile_index += self.vram_bank * TILE_NUM;
            let tile = &self.tiles[tile_index];
            let col = (x - start_x) % TILESIZE;
            let pixel = tile.get_row(row)[col] as usize;
            let color = if mode == GB::CGB {
                gbc2rgba(self.cgb_bg_pal_data[2 * pixel], self.cgb_bg_pal_data[2 * pixel + 1])
            } else {
                dmg_pal[pal_indices[pixel] as usize]
            };

            for i in 0..COLOR_CHANNELS {
                pixel_row[COLOR_CHANNELS * x + i] = color[i];
            }
        }

        // The window layer has an odd edge case
        // If it is disabled mid-frame and then re-enabled, it continues rendering where it was
        // Thus, we need to keep track of what scanline we finished rendering in case we are disabled
        // And continue there if re-enabled this frame (and reset this value at start of next)
        self.last_wndw_line = Some(line);
    }

    /// ```
    /// Render Sprite Line
    ///
    /// Renders the given scanline of the sprite layer
    ///
    /// Inputs:
    ///     Array to load pixel data into (&[u8])
    ///     Scanline to render (u8)
    ///     GB hardware type
    /// ```
    fn render_sprite_line(&self, pixel_row: &mut [u8], line: u8, mode: GB) {
        // Iterate through every sprite
        let sorted_sprites = self.sort_sprites();
        let is_8x16 = self.spr_are_8x16();
        let screen_coords = self.get_scroll_coords();
        let lcd_control = self.read_io(LCDC);
        let mut sprites_drawn = 0;
        for spr in sorted_sprites {
            if !spr.contains_scanline(line, is_8x16) || !spr.is_onscreen() {
                continue;
            }

            sprites_drawn += 1;
            // System only allows finite number of sprites drawn per line
            // If we hit threshold, no more sprites can be drawn on this line

            // TODO: This has been shown to cause issues on GBC games (See Mario Deluxe)
            // Need to re-verify whether this is a requirement there as well
            if sprites_drawn > SPR_PER_LINE && mode != GB::CGB {
                break;
            }

            let dmg_pal = self.palette.get_spr_pal(spr.get_pal());
            let pal_indices = self.get_dmg_spr_indices(spr.get_pal());
            let cgb_colors = self.get_cgb_spr_indices(spr.get_pal());
            let mut above_bg = spr.is_above_bkgd();
            let (top_x, top_y) = spr.get_coords();
            // Get which row in the sprite we're drawing
            let row = ((line as i16) - top_y) as usize;
            // If sprite is Y-flipped, adjust row
            let row = if spr.is_y_flip() {
                if is_8x16 {
                    (2 * TILESIZE) - row - 1
                } else {
                    TILESIZE - row - 1
                }
            } else {
                row
            };

            let spr_num = if is_8x16 {
                // In 8x16 mode, lower bit of tile number is ignored
                // Upper 8x8 tile is NN & $FE
                // Lower 8x8 tile is NN | $01
                if row < TILESIZE {
                    spr.get_tile_num() & 0xFE
                } else {
                    spr.get_tile_num() | 0x01
                }
            } else {
                // If 8x8 sprite, simply get tile num
                spr.get_tile_num()
            };
            let spr_bank = spr_num as usize + (spr.get_vram_bank() * TILE_NUM);

            let tile = &self.tiles[spr_bank];
            let pixels = tile.get_row(row % TILESIZE);
            let spr_x = top_x as usize;
            for col in 0..TILESIZE {
                let pixel = pixels[col as usize] as usize;
                let x_offset = if spr.is_x_flip() {
                    TILESIZE - col - 1
                } else {
                    col
                };

                let pixel_x = spr_x.wrapping_add(x_offset);
                // Move on if pixel is going to be drawn off-screen
                if pixel_x >= SCREEN_WIDTH {
                    continue;
                }

                let pixel_rgba = &pixel_row[(COLOR_CHANNELS * pixel_x)..(COLOR_CHANNELS * (pixel_x + 1))];
                let bkgd_transparent = if mode == GB::CGB {
                    // Need to get the specific palette for this background tile
                    let map_x = (screen_coords.x as usize + pixel_x) % MAP_SIZE;
                    let map_y = ((screen_coords.y as usize) + (line as usize)) % MAP_SIZE;
                    // The index is the cell in question, plus the offset for which map table is being used
                    let idx = (map_y * MAP_SIZE + map_x) + (self.get_bkgd_tile_map_index() as usize * TILE_MAP_TBL_SIZE);
                    let tile_data = self.tile_maps[idx];
                    let pal_indices = self.get_cgb_bg_indices(tile_data.get_pal_num());
                    let bkgd_pal = gbc2rgba(pal_indices[0], pal_indices[1]);

                    // While we have the background tile metadata, see if this tile has priority over sprites
                    above_bg &= !tile_data.is_bg_priority();
                    // Master enable, if LCDC.0 cleared, then sprites always display on top
                    above_bg |= !lcd_control.get_bit(BG_DISP_BIT);

                    // Check if the pixel already drawn is the transparancy color for that BG tile
                    pixel_rgba == bkgd_pal
                } else {
                    pixel_rgba == dmg_pal[0]
                };

                // Only draw pixel if
                // - Pixel isn't transparent
                // - Sprite is going to be drawn above the backgroud OR
                // - Sprite is below background, but background has transparent color here
                if pixel != 0 && (above_bg || bkgd_transparent) {
                    let color = if mode == GB::CGB {
                        gbc2rgba(cgb_colors[2 * pixel], cgb_colors[2 * pixel + 1])
                    } else {
                        dmg_pal[pal_indices[pixel] as usize]
                    };

                    for i in 0..COLOR_CHANNELS {
                        pixel_row[COLOR_CHANNELS * pixel_x + i] = color[i];
                    }
                }
            }
        }
    }

    /// ```
    /// Write IO
    ///
    /// Writes byte to I/O register space ($FF00-$FF7F)
    ///
    /// Inputs:
    ///     Address to write to (u16)
    ///     Value to write (u8)
    /// ```
    fn write_io(&mut self, addr: u16, val: u8) {
        let io_addr = addr - IO_START;
        self.io[io_addr as usize] = val;
    }

    /// ```
    /// Read IO
    ///
    /// Reads byte from I/O register space ($FF00-$FF7F)
    ///
    /// Input:
    ///     Address to read from (u16)
    ///
    /// Output:
    ///     Value at address (u8)
    /// ```
    fn read_io(&self, addr: u16) -> u8 {
        let io_addr = addr - IO_START;
        self.io[io_addr as usize]
    }

    /// ```
    /// Get DMG background indices
    ///
    /// Gets the palette indices from the BGP register ($FF47)
    ///
    /// Output:
    ///     Palette indices ([u8])
    /// ```
    fn get_dmg_bg_indices(&self) -> [u8; DMG_PAL_SIZE] {
        unpack_u8(self.read_io(BGP))
    }

    /// ```
    /// Get CGB background indices
    ///
    /// Gets the slice of currently used GBC palette data
    ///
    /// Input:
    ///     Which palette is being used (usize)
    ///
    /// Output:
    ///     Slice of palette data (&[u8])
    /// ```
    fn get_cgb_bg_indices(&self, num: usize) -> &[u8] {
        &self.cgb_bg_pal_data[(num * CGB_PAL_SIZE)..((num + 1) * CGB_PAL_SIZE)]
    }

    /// ```
    /// Get sprite indices
    ///
    /// Gets the palette indices for the sprites
    ///
    /// Input:
    ///     Which DMG palette to use (u8)
    ///
    /// Output:
    ///     Palette indices ([u8])
    /// ```
    fn get_dmg_spr_indices(&self, pal: u8) -> [u8; DMG_PAL_SIZE] {
        match pal {
            0 => { unpack_u8(self.read_io(OBP0)) },
            1 => { unpack_u8(self.read_io(OBP1)) },
            _ => {
                // This won't be used by non-DMG, but need to return some value
                [0; DMG_PAL_SIZE]
            }
        }
    }

    /// ```
    /// Get CGB sprite indices
    ///
    /// Gets palette indices for GBC sprites
    ///
    /// Input:
    ///     Which CGB palette to use (u8)
    ///
    /// Output:
    ///     Palette data (&[u8])
    /// ```
    fn get_cgb_spr_indices(&self, pal: u8) -> &[u8] {
        &self.cgb_spr_pal_data[(pal as usize * CGB_PAL_SIZE)..((pal + 1) as usize * CGB_PAL_SIZE)]
    }

    /// ```
    /// Sort sprites
    ///
    /// Sort sprites into correct drawing order
    ///
    /// Output:
    ///     Sorted sprites (Vec<Sprite>)
    /// ```
    fn sort_sprites(&self) -> Vec<Sprite> {
        // In event of overlap, sprites are drawn
        // (on DMG) with the lowest x-coordinate on top.
        // If tie, lowest sprite number goes on top
        let mut sprites = self.oam.to_vec();
        // Reverse the vector so that lower sprite number is earlier in a tie
        sprites.reverse();
        sprites.sort_by(|a, b| b.get_coords().0.cmp(&a.get_coords().0));
        sprites
    }

    /// ```
    /// Is the LCD displayed
    ///
    /// Is the LCD screen enabled
    ///
    /// Output:
    ///     Whether or not LCD screen is enabled (bool)
    /// ```
    fn is_lcd_dspl(&self) -> bool {
        let lcd_control = self.read_io(LCDC);
        lcd_control.get_bit(LCD_DISP_BIT)
    }

    /// ```
    /// Is background displayed
    ///
    /// Is background layer currently visible
    ///
    /// Output:
    ///     Whether or not background is displayed (bool)
    /// ```
    fn is_bkgd_dspl(&self, mode: GB) -> bool {
        if mode != GB::CGB {
            let lcd_control = self.read_io(LCDC);
            lcd_control.get_bit(BG_DISP_BIT)
        } else {
            true
        }
    }

    /// ```
    /// Is window displayed
    ///
    /// Is the window layer currently visible
    ///
    /// Output:
    ///     Whether window layer is visible (bool)
    /// ```
    fn is_wndw_dspl(&self) -> bool {
        let lcd_control = self.read_io(LCDC);
        lcd_control.get_bit(WNDW_DISP_BIT)
    }

    /// ```
    /// Are sprites displayed
    ///
    /// Is the sprite layer visible
    ///
    /// Output:
    ///     Whether the sprite layer is visible (bool)
    /// ```
    fn is_sprt_dspl(&self) -> bool {
        let lcd_control = self.read_io(LCDC);
        lcd_control.get_bit(SPR_DISP_BIT)
    }

    /// ```
    /// Get background tileset index
    ///
    /// Returns which tileset is being used (0/1)
    ///
    /// Output:
    ///     Tileset index (u8)
    /// ```
    fn get_bkgd_wndw_tile_set_index(&self) -> u8 {
        let lcd_control = self.read_io(LCDC);
        if lcd_control.get_bit(BG_WNDW_TILE_DATA_BIT) { 1 } else { 0 }
    }

    /// ```
    /// Get background tilemap index
    ///
    /// Returns which tilemap set is being used (0/1)
    ///
    /// Output:
    ///     Tilemap index (u8)
    /// ```
    fn get_bkgd_tile_map_index(&self) -> u8 {
        let lcd_control = self.read_io(LCDC);
        if lcd_control.get_bit(BG_TILE_MAP_BIT) { 1 } else { 0 }
    }

    /// ```
    /// Get window tilemap index
    ///
    /// Returns which window tilemap set is being used (0/1)
    ///
    /// Output:
    ///     Tilemap index (u8)
    /// ```
    fn get_wndw_tile_map_index(&self) -> u8 {
        let lcd_control = self.read_io(LCDC);
        if lcd_control.get_bit(WNDW_TILE_MAP_BIT) { 1 } else { 0 }
    }

    /// ```
    /// Are sprites 8x16?
    ///
    /// Returns true if sprites are to be drawn 8x16
    ///
    /// Output:
    ///     Whether spries are 8x16 (vs 8x8) (bool)
    /// ```
    fn spr_are_8x16(&self) -> bool {
        self.read_io(LCDC).get_bit(SPR_SIZE_BIT)
    }

    /// ```
    /// Set VRAM bank
    ///
    /// Sets which VRAM tile bank should be used (either 0 or 1)
    ///
    /// Input:
    ///     Which bank to use (u8)
    /// ```
    fn set_vram_bank(&mut self, val: u8) {
        self.vram_bank = if val.get_bit(0) { 1 } else { 0 };
    }

    /// ```
    /// Get scroll coords
    ///
    /// Returns the values of the SCX and SCY registers
    ///
    /// Output:
    ///     SCX, SCY point (Point)
    /// ```
    fn get_scroll_coords(&self) -> Point<u8> {
        let scroll_x = self.read_io(SCX);
        let scroll_y = self.read_io(SCY);

        Point { x: scroll_x, y: scroll_y }
    }

    /// ```
    /// Get window coords
    ///
    /// Returns the window position from the WX and WY registers
    ///
    /// Output:
    ///     Location of the window (Point)
    /// ```
    fn get_wndw_coords(&self) -> Point<u8> {
        let wndw_x = self.read_io(WX).saturating_sub(7);
        let wndw_y = self.read_io(WY);

        Point{ x: wndw_x, y: wndw_y }
    }

    /// ```
    /// Read CGB Background color data
    ///
    /// Gets the color data from the specified index
    ///
    /// Output:
    ///     Partial color data loaded into the palette data RAM register
    /// ```
    fn read_cgb_bg_color(&self) -> u8 {
        let ind = self.read_io(BGPI) & 0x3F;
        self.cgb_bg_pal_data[ind as usize]
    }

    /// ```
    /// Write CGB Background color data
    ///
    /// Sets the color data from the specified index
    /// Auto-increments if BGPI bit 7 is set
    ///
    /// Input:
    ///     New value for the index set in BGPI
    /// ```
    fn write_cgb_bg_color(&mut self, val: u8) {
        let bgpi = self.read_io(BGPI);
        self.cgb_bg_pal_data[(bgpi & 0x3F) as usize] = val;
        // Auto-increment if bit 7 set
        if bgpi.get_bit(AUTO_INC_BIT) {
            self.write_io(BGPI, (bgpi + 1) & 0b1011_1111);
        }
    }

    /// ```
    /// Read CGB sprite color data
    ///
    /// Gets the color data from the specified index
    ///
    /// Output:
    ///     Partial color data loaded into the palette data RAM register
    /// ```
    fn read_cgb_spr_color(&self) -> u8 {
        let ind = self.read_io(OBPI) & 0x7F;
        self.cgb_spr_pal_data[ind as usize]
    }

    /// ```
    /// Write CGB sprite color data
    ///
    /// Sets the color data from the specified index
    ///
    /// Input:
    ///     New value for the index set in OBPI
    /// ```
    fn write_cgb_spr_color(&mut self, val: u8) {
        let obpi = self.read_io(OBPI);
        self.cgb_spr_pal_data[(obpi & 0x3F) as usize] = val;
        // Auto-increment if bit 7 set
        if obpi.get_bit(AUTO_INC_BIT) {
            self.write_io(OBPI, (obpi + 1) & 0b1011_1111);
        }
    }
}
