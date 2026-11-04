//! 网络图片的磁盘缓存：游戏封面和官方说明图只下载一次，存到 `~/.cache/flydigi-linux/images/`，
//! 之后直接读文件。替代 egui_extras 只缓存在内存里的 http 加载器。

use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use eframe::egui::{
    self, Context,
    load::{Bytes, BytesLoadResult, BytesLoader, BytesPoll, LoadError},
};

enum Entry {
    Pending,
    Ready(Arc<[u8]>),
    Failed(String),
}

pub struct DiskCacheLoader {
    dir: PathBuf,
    entries: Arc<Mutex<HashMap<String, Entry>>>,
}

impl DiskCacheLoader {
    pub fn new() -> Self {
        let cache = std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".cache"));
        Self { dir: cache.join("flydigi-linux/images"), entries: Arc::default() }
    }

    fn path_for(&self, uri: &str) -> PathBuf {
        // FNV-1a，作为稳定的文件名
        let hash = uri.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3));
        self.dir.join(format!("{hash:016x}"))
    }
}

/// 按文件头判断类型，图片地址不一定带扩展名。
fn sniff_mime(bytes: &[u8]) -> Option<String> {
    let mime = if bytes.starts_with(b"\x89PNG") {
        "image/png"
    } else if bytes.starts_with(&[0xFF, 0xD8]) {
        "image/jpeg"
    } else if bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        "image/webp"
    } else if bytes.starts_with(b"GIF8") {
        "image/gif"
    } else {
        return None;
    };
    Some(mime.to_owned())
}

fn ready(bytes: &Arc<[u8]>) -> BytesLoadResult {
    Ok(BytesPoll::Ready { size: None, mime: sniff_mime(bytes), bytes: Bytes::Shared(bytes.clone()) })
}

fn download(uri: &str) -> Result<Vec<u8>, String> {
    let mut body = Vec::new();
    ureq::get(uri)
        .call()
        .map_err(|e| e.to_string())?
        .into_body()
        .into_reader()
        .read_to_end(&mut body)
        .map_err(|e| e.to_string())?;
    Ok(body)
}

impl BytesLoader for DiskCacheLoader {
    fn id(&self) -> &str {
        egui::generate_loader_id!(DiskCacheLoader)
    }

    fn load(&self, ctx: &Context, uri: &str) -> BytesLoadResult {
        if !(uri.starts_with("http://") || uri.starts_with("https://")) {
            return Err(LoadError::NotSupported);
        }
        let mut entries = self.entries.lock().unwrap();
        match entries.get(uri) {
            Some(Entry::Ready(bytes)) => return ready(bytes),
            Some(Entry::Pending) => return Ok(BytesPoll::Pending { size: None }),
            Some(Entry::Failed(e)) => return Err(LoadError::Loading(e.clone())),
            None => {}
        }
        let path = self.path_for(uri);
        if let Ok(data) = std::fs::read(&path) {
            let bytes: Arc<[u8]> = data.into();
            entries.insert(uri.to_owned(), Entry::Ready(bytes.clone()));
            return ready(&bytes);
        }
        entries.insert(uri.to_owned(), Entry::Pending);
        drop(entries);

        let (uri, entries, ctx, dir) = (uri.to_owned(), self.entries.clone(), ctx.clone(), self.dir.clone());
        std::thread::Builder::new()
            .name("image-download".into())
            .spawn(move || {
                let entry = match download(&uri) {
                    Ok(data) => {
                        let tmp = path.with_extension("part");
                        if std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&tmp, &data)).is_ok() {
                            let _ = std::fs::rename(&tmp, &path);
                        }
                        Entry::Ready(data.into())
                    }
                    Err(e) => {
                        log::warn!("下载图片失败 {uri}：{e}");
                        Entry::Failed(e)
                    }
                };
                entries.lock().unwrap().insert(uri, entry);
                ctx.request_repaint();
            })
            .expect("启动图片下载线程");
        Ok(BytesPoll::Pending { size: None })
    }

    fn forget(&self, uri: &str) {
        self.entries.lock().unwrap().remove(uri);
    }

    fn forget_all(&self) {
        self.entries.lock().unwrap().clear();
    }

    fn byte_size(&self) -> usize {
        self.entries
            .lock()
            .unwrap()
            .values()
            .map(|e| match e {
                Entry::Ready(b) => b.len(),
                _ => 0,
            })
            .sum()
    }

    fn has_pending(&self) -> bool {
        self.entries.lock().unwrap().values().any(|e| matches!(e, Entry::Pending))
    }
}
