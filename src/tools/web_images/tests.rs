#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_ddg_vqd() {
        assert_eq!(
            extract_ddg_vqd("foo vqd=\"123-456\" bar"),
            Some("123-456".to_string())
        );
        assert_eq!(extract_ddg_vqd("foo"), None);
    }

    #[test]
    fn detects_png_dimensions() {
        let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        bytes.extend_from_slice(&32u32.to_be_bytes());
        bytes.extend_from_slice(&16u32.to_be_bytes());
        assert_eq!(detect_image_dimensions(&bytes, "image/png"), (32, 16));
    }
}
