//! 文件读写的小工具：原子写、内容指纹、容错读文本。
//!
//! 待办文件被两边共写（本应用 / AI 直接改文件），所以这里的两条约定必须守住：
//!   - 写必须原子（tmp + rename），写到一半崩了也不能留下半截文件；
//!   - 写之前必须能比对「我上次看到的」和「现在盘上的」是不是同一份 —— 靠指纹，不靠 mtime。

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// 原子写：同目录写 `.tmp` 再 rename 覆盖。
///
/// 为什么不用 `write` 直接覆盖：写一半进程崩溃 / 断电，文件会变成半截 JSON，
/// 下一次加载直接解析失败，整份待办就"消失"了（插件时代没做这层，是靠运气）。
pub fn atomic_write(path: &Path, data: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("创建目录失败 {}: {}", parent.display(), e))?;
    }
    let tmp = tmp_path(path);
    let mut f = fs::File::create(&tmp).map_err(|e| format!("创建临时文件失败 {}: {}", tmp.display(), e))?;
    f.write_all(data)
        .map_err(|e| format!("写入临时文件失败 {}: {}", tmp.display(), e))?;
    // 刷盘：rename 之后临时文件就没了，这里不 flush 的话数据可能还在页缓存里
    f.sync_all()
        .map_err(|e| format!("刷盘失败 {}: {}", tmp.display(), e))?;
    drop(f);
    fs::rename(&tmp, path).map_err(|e| format!("替换文件失败 {}: {}", path.display(), e))?;
    Ok(())
}

/// 临时文件路径：`待办.json` → `待办.json.tmp`
/// （不能用 `with_extension`，那会把 `待办.json` 变成 `待办.tmp`）
fn tmp_path(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".to_string());
    name.push_str(".tmp");
    path.with_file_name(name)
}

/// 内容指纹（FNV-1a 64 + 长度）。
///
/// 不用 mtime：同一毫秒内的两次改动区分不出来，而且有些编辑器保存后 mtime 精度只有秒级。
/// 不引入 sha2：这只是"有没有变"的判断，不是安全场景，FNV 够用且零依赖。
pub fn fingerprint(data: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in data {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{:016x}{:08x}", h, data.len())
}

/// 读文本文件：容错非法 UTF-8，并剥掉 BOM。
///
/// BOM 必须剥：AI 或编辑器写文件时可能带上 `\u{feff}`，前端 `JSON.parse` 会直接炸，
/// 表现出来是"待办清单突然空了"，实际文件好着呢。
pub fn read_text(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|e| format!("读取失败 {}: {}", path.display(), e))?;
    let s = String::from_utf8_lossy(&bytes).into_owned();
    Ok(s.trim_start_matches('\u{feff}').to_string())
}

