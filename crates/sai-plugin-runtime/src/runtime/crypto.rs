use super::budget;
use mlua::{Lua, Table, Value};
use sha2::Digest;

const CHUNK_BYTES: usize = 64 * 1024;

/// 【插件】【摘要绑定】安装只处理调用方字节的通用摘要接口，不读取文件或访问宿主服务
/// @param lua 虚拟机；api 为 sai 表；limit 为单次输入字节上限
/// @returns 摘要接口安装结果
pub(super) fn install(lua: &Lua, api: &Table, limit: usize) -> mlua::Result<()> {
    let crypto = lua.create_table()?;
    crypto.set(
        "digest",
        lua.create_function(move |lua, (algorithm, data): (Value, Value)| {
            let (Value::String(algorithm), Value::String(data)) = (algorithm, data) else {
                return Err(mlua::Error::runtime(
                    "digest requires string algorithm and bytes",
                ));
            };
            let bytes = data.as_bytes();
            if bytes.len() > limit {
                return Err(mlua::Error::runtime(
                    "crypto input exceeds plugin size limit",
                ));
            }
            if algorithm.as_bytes().len() > 32 {
                return Err(mlua::Error::runtime("unsupported digest algorithm"));
            }
            budget::charge_bytes(lua, bytes.len())?;
            let output = digest(lua, &algorithm.to_str()?, &bytes)?;
            budget::checkpoint(lua)?;
            Ok(output)
        })?,
    )?;
    api.set("crypto", crypto)
}

/// 【插件】【字节摘要】使用规范算法标识计算小写十六进制结果，业务别名由插件解析
/// @param lua 虚拟机；algorithm 为规范算法名；data 为原始字节
/// @returns 摘要文本；未知算法或执行失效时返回错误
fn digest(lua: &Lua, algorithm: &str, data: &[u8]) -> mlua::Result<String> {
    match algorithm {
        "md5" => {
            let mut state = md5::Context::new();
            update_chunks(lua, data, |chunk| state.consume(chunk))?;
            Ok(format!("{:x}", state.compute()))
        }
        "sha1" => digest_with::<sha1::Sha1>(lua, data),
        "sha224" => digest_with::<sha2::Sha224>(lua, data),
        "sha256" => digest_with::<sha2::Sha256>(lua, data),
        "sha384" => digest_with::<sha2::Sha384>(lua, data),
        "sha512" => digest_with::<sha2::Sha512>(lua, data),
        "sha3_224" => digest_with::<sha3::Sha3_224>(lua, data),
        "sha3_256" => digest_with::<sha3::Sha3_256>(lua, data),
        "sha3_384" => digest_with::<sha3::Sha3_384>(lua, data),
        "sha3_512" => digest_with::<sha3::Sha3_512>(lua, data),
        "blake2b" => digest_with::<blake2::Blake2b512>(lua, data),
        "blake2s" => digest_with::<blake2::Blake2s256>(lua, data),
        "blake3" => {
            let mut state = blake3::Hasher::new();
            update_chunks(lua, data, |chunk| {
                state.update(chunk);
            })?;
            Ok(state.finalize().to_hex().to_string())
        }
        "crc32" => {
            let mut state = crc32fast::Hasher::new();
            update_chunks(lua, data, |chunk| state.update(chunk))?;
            Ok(format!("{:08x}", state.finalize()))
        }
        "adler32" => {
            let (mut a, mut b) = (1_u32, 0_u32);
            update_chunks(lua, data, |chunk| {
                for byte in chunk {
                    a = (a + u32::from(*byte)) % 65521;
                    b = (b + a) % 65521;
                }
            })?;
            Ok(format!("{:08x}", (b << 16) | a))
        }
        _ => Err(mlua::Error::runtime("unsupported digest algorithm")),
    }
}

/// 【插件】【分块摘要】统一执行实现 Digest 协议的算法，分块检查取消与截止时间
/// @param lua 虚拟机；data 为原始字节；D 为具体摘要算法
/// @returns 小写十六进制摘要
fn digest_with<D: Digest>(lua: &Lua, data: &[u8]) -> mlua::Result<String> {
    let mut state = D::new();
    update_chunks(lua, data, |chunk| state.update(chunk))?;
    Ok(hex::encode(state.finalize()))
}

/// 【插件】【计算分块】原生摘要每处理最多 64 KiB 检查一次调用状态
/// @param lua 虚拟机；data 为完整字节；update 为算法的增量更新函数
/// @returns 全部块处理结果，失效时立即停止后续计算
fn update_chunks(lua: &Lua, data: &[u8], mut update: impl FnMut(&[u8])) -> mlua::Result<()> {
    for chunk in data.chunks(CHUNK_BYTES) {
        budget::checkpoint(lua)?;
        update(chunk);
    }
    Ok(())
}
