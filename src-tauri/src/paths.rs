use std::path::{Path, PathBuf};
use std::fs;

pub fn user_data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("STUDIO_USER_DATA") {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }

    let app_data = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    let current = app_data.join("De Story");
    let legacy_bang = app_data.join("Bang Story");
    let legacy = app_data.join("Studio Cerita");

    // Legacy folder migration: if old folder exists and new one does not, rename it
    if !current.exists() && legacy_bang.join("studio.db").exists() {
        if fs::rename(&legacy_bang, &current).is_err() {
            return legacy_bang;
        }
    } else if !current.exists() && legacy.join("studio.db").exists() {
        if fs::rename(&legacy, &current).is_err() {
            return legacy;
        }
    }

    let _ = fs::create_dir_all(&current);
    current
}

pub fn db_path() -> PathBuf {
    user_data_dir().join("studio.db")
}

pub fn projects_root() -> PathBuf {
    let p = user_data_dir().join("projects");
    let _ = fs::create_dir_all(&p);
    p
}

pub fn project_dir(id: &str) -> PathBuf {
    let p = projects_root().join(id);
    let _ = fs::create_dir_all(&p);
    p
}

pub fn asset_abs_path(project_id: &str, local_path: &str) -> PathBuf {
    projects_root().join(project_id).join(local_path)
}

pub fn asset_url(project_id: &str, local_path: &str) -> String {
    let clean = local_path.replace('\\', "/");
    let encoded: Vec<String> = clean
        .split('/')
        .map(|seg| urlencoding::encode(seg).into_owned())
        .collect();
    let path = format!("asset/{}/{}", urlencoding::encode(project_id), encoded.join("/"));
    // WebView2 (Windows) and Android only intercept custom protocols rewritten to
    // {http}://{scheme}.localhost — see wry's custom_protocol_workaround. A raw
    // studio:// subresource URL in an <img>/@font-face never matches the filter, so
    // the handler is never called and the resource renders as broken. On Windows the
    // browser-facing URL must therefore be the HTTP workaround form; the handler
    // still receives studio://localhost/... after wry reverts it.
    if cfg!(windows) {
        format!("http://studio.localhost/{path}")
    } else {
        format!("studio://{path}")
    }
}

#[cfg(test)]
mod tests {
    use super::asset_url;

    #[test]
    fn asset_url_uses_browser_compatible_origin() {
        let url = asset_url("project-1", "images/sheet.jpg");
        if cfg!(windows) {
            assert_eq!(url, "http://studio.localhost/asset/project-1/images/sheet.jpg");
        } else {
            assert_eq!(url, "studio://asset/project-1/images/sheet.jpg");
        }
    }

    #[test]
    fn asset_url_encodes_each_path_segment() {
        let url = asset_url("project #1", "images\\kucing 100% #1?.jpg");
        assert!(url.ends_with("/asset/project%20%231/images/kucing%20100%25%20%231%3F.jpg"));
    }

    #[test]
    fn asset_url_preserves_unicode_as_encoded_utf8() {
        let url = asset_url("proyek", "images/kucing 🐈.jpg");
        assert!(url.ends_with("/asset/proyek/images/kucing%20%F0%9F%90%88.jpg"));
    }
}

pub fn fonts_dir(resource_dir: Option<&Path>) -> PathBuf {
    // 1. If resource_dir provided (production bundle)
    if let Some(res) = resource_dir {
        let p = res.join("resources").join("fonts");
        if p.exists() {
            return p;
        }
        let p2 = res.join("fonts");
        if p2.exists() {
            return p2;
        }
    }
    // 2. Dev mode relative path
    let dev_fonts = PathBuf::from("resources/fonts");
    if dev_fonts.exists() {
        return dev_fonts.canonicalize().unwrap_or(dev_fonts);
    }
    let dev_parent = PathBuf::from("../resources/fonts");
    if dev_parent.exists() {
        return dev_parent.canonicalize().unwrap_or(dev_parent);
    }
    PathBuf::from("resources/fonts")
}

pub fn whisper_dir() -> PathBuf {
    let p = user_data_dir().join("models").join("whisper");
    let _ = fs::create_dir_all(&p);
    p
}

pub fn whisper_model_path() -> PathBuf {
    whisper_dir().join("ggml-small.bin")
}

pub fn whisper_cli_dir(resource_dir: Option<&Path>) -> PathBuf {
    if let Some(res) = resource_dir {
        let p = res.join("resources").join("whisper");
        if p.exists() {
            return p;
        }
        let p2 = res.join("whisper");
        if p2.exists() {
            return p2;
        }
    }
    let dev = PathBuf::from("resources/whisper");
    if dev.exists() {
        return dev.canonicalize().unwrap_or(dev);
    }
    let dev_parent = PathBuf::from("../resources/whisper");
    if dev_parent.exists() {
        return dev_parent.canonicalize().unwrap_or(dev_parent);
    }
    PathBuf::from("resources/whisper")
}

pub fn ffmpeg_path(resource_dir: Option<&Path>) -> PathBuf {
    if let Ok(p) = std::env::var("FFMPEG_PATH") {
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    if let Ok(path) = which::which("ffmpeg") {
        return path;
    }
    if let Some(res) = resource_dir {
        for sub in &["resources/ffmpeg.exe", "resources/bin/ffmpeg.exe", "ffmpeg.exe", "bin/ffmpeg.exe"] {
            let p = res.join(sub);
            if p.exists() {
                return p;
            }
        }
    }
    for dev in &["resources/ffmpeg.exe", "../resources/ffmpeg.exe", "node_modules/ffmpeg-static/ffmpeg.exe", "../node_modules/ffmpeg-static/ffmpeg.exe", r"C:\Program Files\ffmpeg\bin\ffmpeg.exe"] {
        let p = PathBuf::from(dev);
        if p.exists() {
            return p.canonicalize().unwrap_or(p);
        }
    }
    PathBuf::from("ffmpeg.exe")
}
