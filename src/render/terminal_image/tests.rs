#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_iterm_terminal_program() {
        // 用线程局部覆盖而不是改环境变量：并行测试共享进程环境，
        // 改动会被同进程的其它渲染测试看到
        test_override::set(None, Some(true), None);
        assert!(supports_iterm_inline_image());
        test_override::set(None, None, None);
    }

    #[test]
    fn wezterm_prefers_iterm_protocol_over_kitty() {
        test_override::set(Some(false), Some(true), None);
        assert!(!supports_kitty_graphics());
        assert!(supports_iterm_inline_image());
        test_override::set(None, None, None);
    }

    #[test]
    fn kitty_payload_includes_cell_size_and_reserves_rows() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("sample.png");
        // 16x32 像素 + 默认 8x16 单元格 => 至少 2 列 2 行
        let pixels = std::iter::repeat(Rgba {
            r: 255,
            g: 0,
            b: 0,
            a: 255,
        })
        .take(16 * 32)
        .collect::<Vec<_>>();
        write_test_rgba_png(&path, 16, 32, &pixels);
        let output = render_kitty_image(&path).unwrap();
        assert!(output.contains("\x1b_Ga=p,q=2,C=1,i="));
        assert!(output.contains(",p="));
        assert!(output.contains(",c="));
        // 数据传输与放置分离：放置序列不携带 base64 载荷
        assert!(!output.contains("a=T"));
        assert!(!output.contains("f=100"));
        // c/r 同传：终端渲染行数与占位换行数强制一致，图片定位不再漂移
        let declared_rows = output
            .split(",r=")
            .nth(1)
            .and_then(|rest| {
                rest.split(|ch: char| !ch.is_ascii_digit())
                    .next()
                    .and_then(|value| value.parse::<usize>().ok())
            })
            .expect("kitty payload should declare r");
        assert!(output.ends_with('\n'));
        let trailing_newlines = output.chars().rev().take_while(|ch| *ch == '\n').count();
        assert_eq!(
            trailing_newlines, declared_rows,
            "reserved newlines must match declared rows"
        );
    }

    #[test]
    fn encode_kitty_png_supports_explicit_cells_without_newlines() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("sample.png");
        write_test_rgba_png(
            &path,
            2,
            2,
            &[
                Rgba {
                    r: 255,
                    g: 0,
                    b: 0,
                    a: 255,
                },
                Rgba {
                    r: 0,
                    g: 255,
                    b: 0,
                    a: 255,
                },
                Rgba {
                    r: 0,
                    g: 0,
                    b: 255,
                    a: 255,
                },
                Rgba {
                    r: 255,
                    g: 255,
                    b: 255,
                    a: 255,
                },
            ],
        );
        let output = encode_kitty_png(&path, Some(8), Some(3)).unwrap();
        assert!(output.contains("\x1b_Ga=p,q=2,C=1,i="));
        assert!(output.contains(",c=8,r=3"));
        assert!(!output.ends_with('\n'));
    }

    #[test]
    fn detects_windows_terminal_session() {
        test_override::set(None, None, Some(true));
        assert!(supports_windows_terminal_sixel());
        test_override::set(None, None, None);
    }

    #[test]
    fn renders_png_with_ansi_halfblock_fallback() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("sample.png");
        write_test_rgba_png(
            &path,
            2,
            2,
            &[
                Rgba {
                    r: 255,
                    g: 0,
                    b: 0,
                    a: 255,
                },
                Rgba {
                    r: 0,
                    g: 255,
                    b: 0,
                    a: 255,
                },
                Rgba {
                    r: 0,
                    g: 0,
                    b: 255,
                    a: 255,
                },
                Rgba {
                    r: 255,
                    g: 255,
                    b: 255,
                    a: 255,
                },
            ],
        );
        let output = render_ansi_halfblock_image(&path, &TerminalImageSize::default()).unwrap();
        assert!(output.contains("\x1b[38;2;255;0;0m"));
        assert!(output.contains('▀') || output.contains('▄'));
    }

    #[test]
    fn renders_jpeg_with_ansi_halfblock_fallback() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("sample.jpg");
        let file = File::create(&path).unwrap();
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(file, 90);
        encoder
            .encode(
                &[255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255],
                2,
                2,
                image::ExtendedColorType::Rgb8,
            )
            .unwrap();
        let output = render_ansi_halfblock_image(&path, &TerminalImageSize::default()).unwrap();
        assert!(output.contains("\x1b[38;2;"));
        assert!(output.contains('▀') || output.contains('▄'));
    }

    #[test]
    fn renders_png_with_builtin_sixel_protocol() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("sample.png");
        write_test_rgba_png(
            &path,
            2,
            2,
            &[
                Rgba {
                    r: 255,
                    g: 0,
                    b: 0,
                    a: 255,
                },
                Rgba {
                    r: 0,
                    g: 255,
                    b: 0,
                    a: 255,
                },
                Rgba {
                    r: 0,
                    g: 0,
                    b: 255,
                    a: 255,
                },
                Rgba {
                    r: 255,
                    g: 255,
                    b: 255,
                    a: 255,
                },
            ],
        );
        let output = render_sixel_image(&path, &TerminalImageSize::default()).unwrap();
        assert!(output.starts_with("\x1bPq"));
        assert!(output.contains("\"1;1;"));
        assert!(output.contains("#180;2;100;0;0"));
        assert!(output.ends_with("\x1b\\\n"));
    }

    #[test]
    fn sixel_omits_fully_transparent_pixels() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("transparent.png");
        write_test_rgba_png(
            &path,
            2,
            1,
            &[
                Rgba {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 0,
                },
                Rgba {
                    r: 255,
                    g: 0,
                    b: 0,
                    a: 255,
                },
            ],
        );
        let output = render_sixel_image(&path, &TerminalImageSize::default()).unwrap();
        assert!(!output.contains("#0;2;0;0;0"));
        assert!(output.contains("#180;2;100;0;0"));
    }

    #[test]
    fn table_image_dimensions_width_first_keeps_aspect() {
        // 图 240x48，格 10x20：限制 12 列
        // base_r(12)=2；c/r 同传后无需安全余量
        let (cols, rows) = table_image_cell_dimensions(240, 48, 10, 20, 12, 16);
        assert_eq!(cols, 12);
        assert_eq!(rows, 2);
    }

    #[test]
    fn table_image_dimensions_natural_size_without_clamp() {
        // 图 240x48，格 10x20：自然 24 列
        // base_r(24)=3；无安全余量
        let (cols, rows) = table_image_cell_dimensions(240, 48, 10, 20, 48, 16);
        assert_eq!(cols, 24);
        assert_eq!(rows, 3);
    }

    #[test]
    fn table_image_dimensions_height_cap_recomputes_cols() {
        // 100x400，格 10x20，max_rows=4；收 1 列后行高仍不超过上限
        let (cols, rows) = table_image_cell_dimensions(100, 400, 10, 20, 20, 4);
        assert!(rows <= 4);
        assert!(cols >= 1 && cols <= 20);
    }

    #[test]
    fn table_image_dimensions_clamps_abnormal_cell_height() {
        // ioctl 给过大格高 40（宽 10 → 比 4:1）：应钳到 2:1 的 20，行数变多
        let (cols_bad, rows_bad) = table_image_cell_dimensions(240, 48, 10, 40, 48, 16);
        let (cols_ok, rows_ok) = table_image_cell_dimensions(240, 48, 10, 20, 48, 16);
        assert_eq!(cols_bad, cols_ok);
        assert_eq!(rows_bad, rows_ok);
        assert!(rows_ok >= 3);
    }

    #[test]
    fn crop_transparent_bounds_removes_empty_margins() {
        // 8x8 画布，仅右下 2x2 不透明
        let mut pixels = vec![
            Rgba {
                r: 0,
                g: 0,
                b: 0,
                a: 0,
            };
            64
        ];
        for y in 6..8 {
            for x in 6..8 {
                pixels[y * 8 + x] = Rgba {
                    r: 255,
                    g: 0,
                    b: 0,
                    a: 255,
                };
            }
        }
        let image = RasterImage {
            pixels,
            width: 8,
            height: 8,
        };
        let cropped = crop_transparent_bounds(&image);
        // pad=1：内容 [6,7] 向外扩到 [5,7] => 3x3
        assert_eq!(cropped.width, 3);
        assert_eq!(cropped.height, 3);
        assert!(cropped.pixels.iter().any(|p| p.a >= ANSI_ALPHA_THRESHOLD));
    }

    #[test]
    fn kitty_render_uses_cropped_content_not_full_transparent_canvas() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("wide.png");
        // 64x16 画布，左侧 8x8 不透明；裁剪后应明显小于原宽
        let mut pixels = vec![
            Rgba {
                r: 0,
                g: 0,
                b: 0,
                a: 0,
            };
            64 * 16
        ];
        for y in 0..8 {
            for x in 0..8 {
                pixels[y * 64 + x] = Rgba {
                    r: 0,
                    g: 255,
                    b: 0,
                    a: 255,
                };
            }
        }
        write_test_rgba_png(&path, 64, 16, &pixels);
        let output = render_kitty_image(&path).unwrap();
        assert!(output.contains("\x1b_Ga=p,q=2,C=1,i="));
        assert!(output.contains(",c="));
        let c_value = output
            .split(",c=")
            .nth(1)
            .and_then(|rest| rest.split(',').next())
            .and_then(|value| value.parse::<usize>().ok())
            .expect("c= value");
        assert!(
            c_value <= 4,
            "expected cropped kitty width, got c={c_value}"
        );
    }

    #[test]
    fn kitty_render_declares_matching_cols_and_rows() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("gantt_like.png");
        // 宽短图：模拟甘特图，不应在下方预留远超内容的空白行
        let width = 400u32;
        let height = 120u32;
        let pixels = std::iter::repeat(Rgba {
            r: 200,
            g: 200,
            b: 220,
            a: 255,
        })
        .take((width * height) as usize)
        .collect::<Vec<_>>();
        write_test_rgba_png(&path, width, height, &pixels);
        let output = render_kitty_image(&path).unwrap();
        assert!(output.contains(",c="));
        assert!(output.contains(",r="));
        let trailing = output.chars().rev().take_while(|ch| *ch == '\n').count();
        // 默认 8x16：高 120 => 约 8 行；允许少量取整偏差
        assert!(
            trailing <= 12,
            "expected compact row reservation for short image, got {trailing} newlines"
        );
        assert!(trailing >= 1);
    }

    #[test]
    fn pad_raster_to_cell_grid_snaps_to_exact_grid_pixels() {
        // 20x18 像素，格 8x16 => 3 列 2 行网格 = 24x32 像素
        let image = RasterImage {
            pixels: vec![
                Rgba {
                    r: 255,
                    g: 0,
                    b: 0,
                    a: 255,
                };
                20 * 18
            ],
            width: 20,
            height: 18,
        };
        let padded = pad_raster_to_cell_grid(image, 3, 2, 8, 16);
        assert_eq!(padded.width, 24);
        assert_eq!(padded.height, 32);
        // 原始内容保持左上对齐
        assert_eq!(padded.pixels[0].a, 255);
        // 右下补齐区域为透明
        assert_eq!(padded.pixels[padded.pixels.len() - 1].a, 0);
    }

    #[test]
    fn kitty_cell_dimensions_match_aspect_not_independent_ceil() {
        // 400x100，格 8x16：若宽高各自 ceil 会得到 (50, 7)
        // 按比例从 c 推 r 也应接近 7，且不会无故变成接近 max_rows
        let (cols, rows) = kitty_cell_dimensions(400, 100);
        assert!(cols >= 1);
        assert!(rows >= 1);
        // 行数应与比例一致：rows ≈ cols * 8 * 100 / (400 * 16) = cols / 8
        let expected = (cols * 8 * 100).div_ceil(400 * 16).max(1);
        assert_eq!(rows, expected);
    }

    fn write_test_rgba_png(path: &Path, width: u32, height: u32, pixels: &[Rgba]) {
        let file = File::create(path).unwrap();
        let writer = BufWriter::new(file);
        let mut encoder = png::Encoder::new(writer, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        let bytes = pixels
            .iter()
            .flat_map(|pixel| [pixel.r, pixel.g, pixel.b, pixel.a])
            .collect::<Vec<_>>();
        writer.write_image_data(&bytes).unwrap();
    }
}
