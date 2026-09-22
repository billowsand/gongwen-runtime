//! TTB v1（tectonicbundle）打包/解包工具。
//! 用法：
//!   ttb-pack dump <ttb> [index-out]   解析头部与索引
//!   ttb-pack unpack <ttb> <out-dir>   解出全部内容文件（剥 resolved/ 前缀）
//!   ttb-pack pack <dir> <out.ttb>     把 mdx 的目录 bundle 打成 ttb
//!
//! 格式依据 tectonic_bundles 0.4.2 的读取端（ttb.rs / ttb_fs.rs）与上一版
//! gongwen-texlive.ttb 的实际布局反推：
//! - 66 字节头：magic "tectonicbundle"(14) + version u32(=1) + index_start u64 +
//!   index_gzip_len u32 + index_real_len u32 + digest 32B（读取端读 70 字节，
//!   但内容偏移全部走 seek，66 与 70 两种写法都兼容；上游生成的是 66）。
//! - 内容区：逐文件 gzip 后顺序拼接，头两个是 ITAR 元数据文件 FILELIST / SEARCH，
//!   第三个是目录 bundle 的 SHA256SUM，其余放在 resolved/ 前缀下。
//! - 末尾是 gzip 压缩的索引文本：[DEFAULTSEARCH]/[SEARCH:MAIN]/[FILELIST]。
//! - digest 只是缓存失效标识（ttb_fs.rs 的 get_digest 原样返回，无人重算校验），
//!   取 sha256(索引文本) 即可，内容变了它一定变。

use anyhow::{bail, Result};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use sha2::{Digest, Sha256};
use std::io::{Read, Seek, SeekFrom, Write};

const HEADER_LEN: usize = 66;
const MAGIC: &[u8; 14] = b"tectonicbundle";
/// 搜索规则与上一版 bundle 保持一致：根目录精确匹配 + resolved/ 前缀匹配。
const SEARCH_TEXT: &str = "/\n/resolved//\n";

fn sha256_hex(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    hex_lower(&h.finalize())
}

fn hex_lower(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn gzip(data: &[u8]) -> Result<Vec<u8>> {
    let mut enc = GzEncoder::new(Vec::new(), Compression::default());
    enc.write_all(data)?;
    Ok(enc.finish()?)
}

fn gunzip(data: &[u8]) -> Result<Vec<u8>> {
    let mut dec = GzDecoder::new(data);
    let mut out = Vec::new();
    dec.read_to_end(&mut out)?;
    Ok(out)
}

/// 把 ttb 的全部内容文件解到目录（剥掉 resolved/ 前缀，跳过 ITAR 元数据）。
/// 供「并集合并」用：新版目录 bundle 缺什么，从旧 ttb 里补回来。
fn unpack(ttb: &str, out_dir: &str) -> Result<()> {
    let mut file = std::fs::File::open(ttb)?;
    let mut raw = [0u8; HEADER_LEN];
    file.read_exact(&mut raw)?;
    if &raw[0..14] != MAGIC {
        bail!("不是 tectonicbundle 文件");
    }
    let index_start = u64::from_le_bytes(raw[18..26].try_into()?);
    let index_gzip_len = u32::from_le_bytes(raw[26..30].try_into()?);
    file.seek(SeekFrom::Start(index_start))?;
    let mut gz = vec![0u8; index_gzip_len as usize];
    file.read_exact(&mut gz)?;
    let index = String::from_utf8(gunzip(&gz)?)?;

    let mut count = 0usize;
    let mut in_filelist = false;
    for line in index.lines() {
        if line.starts_with('[') {
            in_filelist = line == "[FILELIST]";
            continue;
        }
        if !in_filelist || line.trim().is_empty() {
            continue;
        }
        let mut parts = line.splitn(5, ' ');
        let (Some(start), Some(gzip_len), Some(_real), Some(_hash), Some(path)) = (
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
        ) else {
            continue;
        };
        let Some(rel) = path.strip_prefix("resolved/") else {
            continue; // FILELIST / SEARCH / SHA256SUM 等元数据不解
        };
        let start: u64 = start.parse()?;
        let gzip_len: u64 = gzip_len.parse()?;
        file.seek(SeekFrom::Start(start))?;
        let mut blob = vec![0u8; gzip_len as usize];
        file.read_exact(&mut blob)?;
        let bytes = gunzip(&blob)?;
        let dest = std::path::Path::new(out_dir).join(rel);
        std::fs::create_dir_all(dest.parent().unwrap())?;
        std::fs::write(&dest, bytes)?;
        count += 1;
    }
    println!("解出 {count} 个文件到 {out_dir}");
    Ok(())
}

fn dump(ttb: &str, index_out: Option<&str>) -> Result<()> {
    let mut file = std::fs::File::open(ttb)?;
    let mut raw = [0u8; HEADER_LEN];
    file.read_exact(&mut raw)?;
    if &raw[0..14] != MAGIC {
        bail!("不是 tectonicbundle 文件");
    }
    let index_start = u64::from_le_bytes(raw[18..26].try_into()?);
    let index_gzip_len = u32::from_le_bytes(raw[26..30].try_into()?);
    let index_real_len = u32::from_le_bytes(raw[30..34].try_into()?);
    println!("index_start     = {index_start}");
    println!("index_gzip_len  = {index_gzip_len}");
    println!("index_real_len  = {index_real_len}");
    println!("digest (header) = {}", hex_lower(&raw[34..66]));

    file.seek(SeekFrom::Start(index_start))?;
    let mut gz = vec![0u8; index_gzip_len as usize];
    file.read_exact(&mut gz)?;
    let index = gunzip(&gz)?;
    println!("index 解压后 {} 字节（头部记录 {index_real_len}）", index.len());
    println!("sha256(index)   = {}", sha256_hex(&index));

    let text = String::from_utf8(index.clone())?;
    let lines: Vec<&str> = text.lines().collect();
    println!("索引共 {} 行", lines.len());
    for line in &lines {
        if line.starts_with('[') {
            println!("  {line}");
        }
    }
    if let Some(out) = index_out {
        std::fs::write(out, &index)?;
        println!("索引已写出到 {out}");
    }
    Ok(())
}

fn pack(dir: &str, out: &str) -> Result<()> {
    // 收集 TeX 资源文件，统一加上 resolved/ 前缀。SHA256SUM 单独作为根级文件。
    let mut resources: Vec<(String, Vec<u8>)> = Vec::new();
    let mut sha256sum: Option<Vec<u8>> = None;
    for entry in walkdir::WalkDir::new(dir).sort_by_file_name() {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(dir)?
            .to_string_lossy()
            .replace('\\', "/");
        if rel == "SHA256SUM" {
            sha256sum = Some(std::fs::read(entry.path())?);
            continue;
        }
        if rel.starts_with('/') || rel.contains("./") || rel.contains("//") {
            bail!("非法 bundle 路径：{rel}");
        }
        resources.push((format!("resolved/{rel}"), std::fs::read(entry.path())?));
    }
    resources.sort_by(|a, b| a.0.cmp(&b.0));
    let sha256sum = sha256sum.unwrap_or_else(|| b"nohash\n".to_vec());
    println!("共 {} 个资源文件", resources.len());

    // 内嵌 FILELIST（ITAR 格式：`<hash|nohash> <path>`，不含偏移）。FILELIST 与
    // SHA256SUM 自身记 nohash，因此内容不靠迭代求不动点。
    let mut embedded = String::from("nohash FILELIST\n");
    embedded.push_str(&format!("{} SEARCH\n", sha256_hex(SEARCH_TEXT.as_bytes())));
    embedded.push_str("nohash SHA256SUM\n");
    for (path, bytes) in &resources {
        embedded.push_str(&format!("{} {path}\n", sha256_hex(bytes)));
    }

    let mut contents: Vec<(String, Vec<u8>, String)> = vec![
        (
            "FILELIST".to_string(),
            embedded.into_bytes(),
            "nohash".to_string(),
        ),
        (
            "SEARCH".to_string(),
            SEARCH_TEXT.as_bytes().to_vec(),
            sha256_hex(SEARCH_TEXT.as_bytes()),
        ),
        ("SHA256SUM".to_string(), sha256sum, "nohash".to_string()),
    ];
    for (path, bytes) in resources {
        let hash = sha256_hex(&bytes);
        contents.push((path, bytes, hash));
    }

    let mut output = std::io::BufWriter::new(std::fs::File::create(out)?);
    output.write_all(&[0u8; HEADER_LEN])?;
    let mut offset = HEADER_LEN as u64;

    let mut index = String::from("[DEFAULTSEARCH]\nMAIN\n[SEARCH:MAIN]\n/\n/resolved//\n[FILELIST]\n");
    for (path, bytes, hash) in &contents {
        let compressed = gzip(bytes)?;
        index.push_str(&format!(
            "{} {} {} {} {}\n",
            offset,
            compressed.len(),
            bytes.len(),
            hash,
            path
        ));
        output.write_all(&compressed)?;
        offset += compressed.len() as u64;
    }

    let index_gz = gzip(index.as_bytes())?;
    let index_start = offset;
    output.write_all(&index_gz)?;
    output.flush()?;

    let digest = Sha256::new().chain_update(index.as_bytes()).finalize();
    let mut header = Vec::with_capacity(HEADER_LEN);
    header.extend_from_slice(MAGIC);
    header.extend_from_slice(&1u32.to_le_bytes());
    header.extend_from_slice(&index_start.to_le_bytes());
    header.extend_from_slice(&(index_gz.len() as u32).to_le_bytes());
    header.extend_from_slice(&(index.len() as u32).to_le_bytes());
    header.extend_from_slice(&digest);

    let mut file = std::fs::OpenOptions::new().read(true).write(true).open(out)?;
    file.seek(SeekFrom::Start(0))?;
    file.write_all(&header)?;

    println!("已写出 {out}（{} 个内容文件）", contents.len());
    println!("index_start = {index_start}");
    println!("digest      = {}", hex_lower(&digest));
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    match args.as_slice() {
        [_, cmd, ttb] if cmd == "dump" => dump(ttb, None),
        [_, cmd, ttb, out] if cmd == "dump" => dump(ttb, Some(out)),
        [_, cmd, ttb, out_dir] if cmd == "unpack" => unpack(ttb, out_dir),
        [_, cmd, dir, out] if cmd == "pack" => pack(dir, out),
        _ => {
            eprintln!("usage: ttb-pack dump <ttb> [index-out]");
            eprintln!("       ttb-pack unpack <ttb> <out-dir>");
            eprintln!("       ttb-pack pack <dir> <out.ttb>");
            std::process::exit(2);
        }
    }
}
