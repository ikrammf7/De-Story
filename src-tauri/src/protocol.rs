use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use tauri::http::{header, Method, Request, Response, StatusCode};
use crate::paths::{fonts_dir, projects_root};

fn get_mime_type(ext: &str) -> &'static str {
    match ext.to_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "wav" => "audio/wav",
        "mp3" => "audio/mpeg",
        "m4a" => "audio/mp4",
        "ogg" => "audio/ogg",
        "ttf" => "font/ttf",
        _ => "application/octet-stream",
    }
}

fn normalize_for_comparison(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    let cleaned = if let Some(stripped) = s.strip_prefix(r"\\?\") {
        stripped
    } else {
        &s
    };
    PathBuf::from(cleaned.to_lowercase().replace('/', "\\"))
}

pub fn handle_studio_request(
    resource_dir: Option<&Path>,
    request: &Request<Vec<u8>>,
) -> Result<Response<Vec<u8>>, Box<dyn std::error::Error>> {
    // Handle CORS preflight
    if request.method() == Method::OPTIONS {
        return Ok(Response::builder()
            .status(StatusCode::NO_CONTENT)
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .header(header::ACCESS_CONTROL_ALLOW_METHODS, "GET, HEAD, OPTIONS")
            .header(header::ACCESS_CONTROL_ALLOW_HEADERS, "Range, Content-Type, Accept")
            .header(header::ACCESS_CONTROL_EXPOSE_HEADERS, "Content-Range, Content-Length, Accept-Ranges")
            .body(Vec::new())?);
    }

    let uri = request.uri();
    let host = uri.host().unwrap_or("");
    let path = uri.path();

    // Determine category ("fonts" or "asset") and the subpath
    let (category, subpath) = if host == "fonts" || host == "asset" {
        (host, path.trim_start_matches('/'))
    } else {
        let trimmed = path.trim_start_matches('/');
        if let Some(rest) = trimmed.strip_prefix("fonts/") {
            ("fonts", rest)
        } else if let Some(rest) = trimmed.strip_prefix("asset/") {
            ("asset", rest)
        } else {
            (host, trimmed)
        }
    };

    let root = match category {
        "fonts" => fonts_dir(resource_dir),
        "asset" => projects_root(),
        _ => {
            return Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(b"Not found".to_vec())?);
        }
    };

    let decoded_path = urlencoding::decode(subpath).map_err(|e| e.to_string())?;
    let rel_path = decoded_path.trim_start_matches('/');

    let target = root.join(rel_path);

    // Security check: path traversal prevention with Windows UNC tolerance
    let canonical_root = root.canonicalize().unwrap_or_else(|_| root.clone());
    let canonical_target = match target.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            return Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(b"File not found".to_vec())?);
        }
    };

    let norm_root = normalize_for_comparison(&canonical_root);
    let norm_target = normalize_for_comparison(&canonical_target);

    if !norm_target.starts_with(&norm_root) {
        return Ok(Response::builder()
            .status(StatusCode::FORBIDDEN)
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .body(b"Forbidden".to_vec())?);
    }

    let mut file = match File::open(&canonical_target) {
        Ok(f) => f,
        Err(_) => {
            return Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(b"File open error".to_vec())?);
        }
    };

    let metadata = file.metadata()?;
    let total_size = metadata.len();
    let ext = canonical_target
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let mime = get_mime_type(ext);

    // Check for HTTP Range header
    let range_header = request
        .headers()
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok());

    if let Some(range_str) = range_header {
        if let Some(bytes_range) = range_str.strip_prefix("bytes=") {
            let parts: Vec<&str> = bytes_range.split('-').collect();
            let start = if !parts[0].is_empty() {
                parts[0].parse::<u64>().unwrap_or(0)
            } else if parts.len() > 1 && !parts[1].is_empty() {
                let suffix = parts[1].parse::<u64>().unwrap_or(0);
                total_size.saturating_sub(suffix)
            } else {
                0
            };

            let end = if parts.len() > 1 && !parts[1].is_empty() {
                parts[1].parse::<u64>().unwrap_or(total_size - 1).min(total_size - 1)
            } else {
                total_size - 1
            };

            if start >= total_size || start > end {
                return Ok(Response::builder()
                    .status(StatusCode::RANGE_NOT_SATISFIABLE)
                    .header(header::CONTENT_RANGE, format!("bytes */{total_size}"))
                    .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                    .body(Vec::new())?);
            }

            let length = end - start + 1;
            file.seek(SeekFrom::Start(start))?;
            let mut buffer = vec![0u8; length as usize];
            file.read_exact(&mut buffer)?;

            return Ok(Response::builder()
                .status(StatusCode::PARTIAL_CONTENT)
                .header(header::CONTENT_TYPE, mime)
                .header(header::CONTENT_LENGTH, length.to_string())
                .header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{total_size}"))
                .header(header::ACCEPT_RANGES, "bytes")
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .header(header::ACCESS_CONTROL_EXPOSE_HEADERS, "Content-Range, Content-Length, Accept-Ranges")
                .body(buffer)?);
        }
    }

    // Full response
    let mut buffer = Vec::with_capacity(total_size as usize);
    file.read_to_end(&mut buffer)?;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime)
        .header(header::CONTENT_LENGTH, total_size.to_string())
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CACHE_CONTROL, "no-cache")
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(header::ACCESS_CONTROL_EXPOSE_HEADERS, "Content-Range, Content-Length, Accept-Ranges")
        .body(buffer)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_for_comparison() {
        let unc_path = Path::new(r"\\?\E:\Tools\Bang Story\assets\img.png");
        let regular_path = Path::new(r"e:\tools\bang story\assets\img.png");
        let slash_path = Path::new("e:/tools/bang story/assets/img.png");

        assert_eq!(normalize_for_comparison(unc_path), normalize_for_comparison(regular_path));
        assert_eq!(normalize_for_comparison(slash_path), normalize_for_comparison(regular_path));
    }

    #[test]
    fn test_mime_type_resolution() {
        assert_eq!(get_mime_type("png"), "image/png");
        assert_eq!(get_mime_type("jpg"), "image/jpeg");
        assert_eq!(get_mime_type("mp4"), "video/mp4");
        assert_eq!(get_mime_type("wav"), "audio/wav");
        assert_eq!(get_mime_type("ttf"), "font/ttf");
    }

    #[test]
    fn test_windows_workaround_font_request() {
        let root = std::env::temp_dir().join(format!("bang-story-font-test-{}", uuid::Uuid::new_v4()));
        let fonts = root.join("resources").join("fonts");
        std::fs::create_dir_all(&fonts).unwrap();
        std::fs::write(fonts.join("test.ttf"), b"font bytes").unwrap();

        // Wry reverses http://studio.localhost/fonts/... before invoking our handler.
        let request = Request::builder()
            .uri("studio://localhost/fonts/test.ttf")
            .body(Vec::new())
            .unwrap();
        let response = handle_studio_request(Some(&root), &request).unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CONTENT_TYPE], "font/ttf");
        assert_eq!(response.body(), b"font bytes");
        std::fs::remove_dir_all(root).unwrap();
    }
}
