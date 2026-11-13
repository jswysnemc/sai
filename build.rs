use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::PathBuf;

use base64::{engine::general_purpose, Engine as _};

/// 【构建】【词表生成】生成嵌入式分词词表，无参数和返回值
fn main() {
    println!("cargo:rerun-if-changed=assets/o200k_base.tiktoken");
    build_o200k_vocab();
}

fn build_o200k_vocab() {
    let source =
        fs::read_to_string("assets/o200k_base.tiktoken").expect("read o200k_base vocabulary");
    let mut output = Vec::with_capacity(source.len() / 2);
    let mut tokens = HashSet::with_capacity(199_998);
    let mut count = 0usize;
    for (expected_rank, line) in source.lines().enumerate() {
        let mut parts = line.split(' ');
        let token = general_purpose::STANDARD
            .decode(parts.next().expect("vocabulary token"))
            .expect("decode vocabulary token");
        assert!(tokens.insert(token.clone()), "duplicate o200k token");
        let rank = parts
            .next()
            .expect("vocabulary rank")
            .parse::<usize>()
            .expect("parse vocabulary rank");
        assert_eq!(rank, expected_rank, "o200k ranks must be sequential");
        let len = u16::try_from(token.len()).expect("token length fits in u16");
        output.extend_from_slice(&len.to_le_bytes());
        output.extend_from_slice(&token);
        count += 1;
    }
    assert_eq!(count, 199_998, "unexpected o200k vocabulary size");
    assert_eq!(tokens.len(), count, "o200k tokens must be unique");

    let destination =
        PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR")).join("o200k_base.bin");
    fs::write(destination, output).expect("write compact o200k vocabulary");
}
