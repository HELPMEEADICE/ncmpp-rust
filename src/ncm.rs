use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;

use aes::cipher::{generic_array::GenericArray, BlockDecrypt, KeyInit};
use aes::Aes128;
use anyhow::{anyhow, Result};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;

static CORE_KEY_HEX: &str = "687A4852416D736F356B496E62617857";
static META_KEY_HEX: &str = "2331346C6A6B5F215C5D2630553C2728";

fn hex_decode(hex: &str) -> Result<[u8; 16]> {
    let mut bytes = [0u8; 16];
    for i in 0..16 {
        bytes[i] = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16)
            .map_err(|e| anyhow!("hex decode failed: {}", e))?;
    }
    Ok(bytes)
}

fn little_u32(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn pkcs7_pad_size(data: &[u8]) -> usize {
    let pad_len = *data.last().unwrap_or(&0) as usize;
    data.len().saturating_sub(pad_len)
}

fn pkcs7_unpad(data: &[u8]) -> &[u8] {
    let size = pkcs7_pad_size(data);
    &data[..size]
}

fn aes_ecb_decrypt(key: &[u8; 16], data: &[u8]) -> Result<Vec<u8>> {
    let cipher = Aes128::new_from_slice(key).map_err(|_| anyhow!("invalid AES key"))?;
    let mut result = data.to_vec();
    for chunk in result.chunks_exact_mut(16) {
        let block = GenericArray::from_mut_slice(chunk);
        cipher.decrypt_block(block);
    }
    Ok(result)
}

fn build_key_box(key_data: &[u8; 16], key_data_unpad: &[u8]) -> [u8; 256] {
    let _ = key_data;

    let mut key_box: [u8; 256] = {
        let mut arr = [0u8; 256];
        for (i, v) in arr.iter_mut().enumerate() {
            *v = i as u8;
        }
        arr
    };

    let key = &key_data_unpad[17..];
    let key_len = key.len();

    let mut c: u8;
    let mut last_byte: u8 = 0;
    let mut key_offset: usize = 0;

    for i in 0..256 {
        let swap = key_box[i];
        c = swap.wrapping_add(last_byte).wrapping_add(key[key_offset]);
        key_offset += 1;
        if key_offset >= key_len {
            key_offset = 0;
        }
        key_box[i] = key_box[c as usize];
        key_box[c as usize] = swap;
        last_byte = c;
    }

    key_box
}

fn decrypt_chunk(buffer: &mut [u8], key_box: &[u8; 256]) {
    for (i, byte) in buffer.iter_mut().enumerate() {
        let j = (i + 1) & 0xff;
        let idx =
            (key_box[j] as usize + key_box[(key_box[j] as usize + j) & 0xff] as usize) & 0xff;
        *byte ^= key_box[idx];
    }
}

pub fn ncm_dump(path: impl AsRef<Path>, out_dir: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    let out_dir = out_dir.as_ref();

    let core_key = hex_decode(CORE_KEY_HEX)?;
    let meta_key = hex_decode(META_KEY_HEX)?;

    let file = File::open(path)?;
    let mut reader = BufReader::with_capacity(0x8000, file);

    reader.seek(SeekFrom::Start(10))?;

    let mut key_len_buf = [0u8; 4];
    reader.read_exact(&mut key_len_buf)?;
    let key_len = little_u32(&key_len_buf) as usize;

    let mut key_data = vec![0u8; key_len];
    reader.read_exact(&mut key_data)?;
    for b in &mut key_data {
        *b ^= 0x64;
    }

    let key_data_dec = aes_ecb_decrypt(&core_key, &key_data)?;
    let key_data_unpad = pkcs7_unpad(&key_data_dec);
    let key_box = build_key_box(&core_key, key_data_unpad);

    let mut meta_len_buf = [0u8; 4];
    reader.read_exact(&mut meta_len_buf)?;
    let meta_len = little_u32(&meta_len_buf) as usize;

    let mut meta_data = vec![0u8; meta_len];
    reader.read_exact(&mut meta_data)?;
    for b in &mut meta_data {
        *b ^= 0x63;
    }

    let meta_b64 = &meta_data[22..];
    let meta_b64 = std::str::from_utf8(meta_b64)
        .map_err(|e| anyhow!("invalid UTF-8 in metadata: {}", e))?;
    let meta_decoded = STANDARD
        .decode(meta_b64)
        .map_err(|e| anyhow!("base64 decode failed: {}", e))?;

    let meta_dec = aes_ecb_decrypt(&meta_key, &meta_decoded)?;
    let meta_unpad = pkcs7_unpad(&meta_dec);

    let meta_json = std::str::from_utf8(&meta_unpad[6..])
        .map_err(|e| anyhow!("invalid UTF-8 in meta JSON: {}", e))?;
    let doc: serde_json::Value =
        serde_json::from_str(meta_json).map_err(|e| anyhow!("JSON parse failed: {}", e))?;
    let format = doc["format"]
        .as_str()
        .ok_or_else(|| anyhow!("missing 'format' field in metadata"))?;
    let extname = format!(".{}", format);

    reader.seek(SeekFrom::Current(9))?;

    let mut img_len_buf = [0u8; 4];
    reader.read_exact(&mut img_len_buf)?;
    let img_len = little_u32(&img_len_buf) as i64;
    reader.seek(SeekFrom::Current(img_len))?;

    let stem = path
        .file_stem()
        .ok_or_else(|| anyhow!("invalid filename"))?
        .to_string_lossy();
    let output_path = out_dir.join(format!("{}{}", stem, extname));

    let out_file = File::create(&output_path)?;
    let mut writer = BufWriter::with_capacity(0x8000, out_file);

    let mut buffer = vec![0u8; 0x8000];
    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        decrypt_chunk(&mut buffer[..n], &key_box);
        writer.write_all(&buffer[..n])?;
    }
    writer.flush()?;

    Ok(())
}
