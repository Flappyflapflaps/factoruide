//! What the page gives the backend (web/src/host): answers to the page's calls, events, the
//! saved files, HTTP. Each function is imported from the page's `fg` module in the browser;
//! natively (tests) they keep what they are given.

/// An HTTP answer: status, body and the headers the backend reads.
#[derive(Debug, Default)]
pub struct Response {
    pub status: u16,
    pub body: Vec<u8>,
    /// `Retry-After`, when the server says how long to wait.
    pub retry_after: Option<String>,
}

#[cfg(target_os = "wasi")]
#[expect(unsafe_code, reason = "the functions the page gives the module")]
mod imports {
    #[link(wasm_import_module = "fg")]
    unsafe extern "C" {
        pub fn fg_reply(id: u32, ptr: *const u8, len: usize);
        pub fn fg_event(ptr: *const u8, len: usize);
        pub fn fg_persist(op: u32, path: *const u8, path_len: usize, data: *const u8, data_len: usize);
        /// Fills `out` with (body pointer, body length, meta pointer, meta length), buffers given
        /// by `fg_alloc` that the backend owns from then on. Returns the status (0: no answer).
        pub fn fg_fetch(request: *const u8, request_len: usize, body: *const u8, body_len: usize, out: *mut u32)
        -> u32;
    }
}

/// Answer the page's call `id` (a JSON text).
pub fn reply(id: u32, json: &str) {
    #[cfg(target_os = "wasi")]
    #[expect(unsafe_code, reason = "the page reads the text before the call returns")]
    // SAFETY: the text lives until the call returns, and the page only reads it.
    unsafe {
        imports::fg_reply(id, json.as_ptr(), json.len());
    }
    #[cfg(not(target_os = "wasi"))]
    native::REPLIES.lock().unwrap().push((id, json.to_owned()));
}

/// Send the page an event (`{"event": name, "payload": ...}`).
pub fn event(name: &str, payload: &serde_json::Value) {
    let json = serde_json::json!({ "event": name, "payload": payload }).to_string();
    #[cfg(target_os = "wasi")]
    #[expect(unsafe_code, reason = "the page reads the text before the call returns")]
    // SAFETY: the text lives until the call returns, and the page only reads it.
    unsafe {
        imports::fg_event(json.as_ptr(), json.len());
    }
    #[cfg(not(target_os = "wasi"))]
    let _ = json;
}

/// Have the page save a file (`Some`) or remove it (`None`), in the browser's storage.
pub fn persist(path: &str, data: Option<&[u8]>) {
    #[cfg(target_os = "wasi")]
    #[expect(unsafe_code, reason = "the page copies the bytes before the call returns")]
    // SAFETY: the bytes live until the call returns, and the page only reads them.
    unsafe {
        let (op, ptr, len) = data.map_or((1, std::ptr::null(), 0), |d| (0, d.as_ptr(), d.len()));
        imports::fg_persist(op, path.as_ptr(), path.len(), ptr, len);
    }
    #[cfg(not(target_os = "wasi"))]
    let _ = (path, data);
}

/// An HTTP request made by the page for the backend (synchronous: the backend runs in Web
/// Workers). `url` relative to the page, or absolute.
pub fn fetch(method: &str, url: &str, content_type: Option<&str>, body: &[u8]) -> Result<Response, String> {
    #[cfg(target_os = "wasi")]
    {
        let request = serde_json::json!({ "method": method, "url": url, "content_type": content_type }).to_string();
        let mut out = [0u32; 4];
        #[expect(unsafe_code, reason = "the page hands over buffers made by fg_alloc")]
        // SAFETY: the request and body are only read; the answer's buffers are `fg_alloc`'s, of
        // the lengths given, and handed over to us.
        let (status, body, meta) = unsafe {
            let status = imports::fg_fetch(
                request.as_ptr(),
                request.len(),
                body.as_ptr(),
                body.len(),
                out.as_mut_ptr(),
            );
            let take = |ptr: u32, len: u32| {
                if ptr == 0 {
                    Vec::new()
                } else {
                    crate::take(ptr as *mut u8, len as usize)
                }
            };
            (status, take(out[0], out[1]), take(out[2], out[3]))
        };
        let meta: serde_json::Value = serde_json::from_slice(&meta).unwrap_or_default();
        if status == 0 {
            return Err(meta["error"].as_str().unwrap_or("no answer").to_owned());
        }
        Ok(Response {
            status: status as u16,
            body,
            retry_after: meta["retry_after"].as_str().map(str::to_owned),
        })
    }
    #[cfg(not(target_os = "wasi"))]
    {
        let _ = (method, content_type, body);
        native::fetch(url)
    }
}

#[cfg(not(target_os = "wasi"))]
pub mod native {
    use std::sync::Mutex;

    /// Answers given so far (tests read them).
    pub static REPLIES: Mutex<Vec<(u32, String)>> = Mutex::new(Vec::new());
    /// Files the tests serve (url -> body).
    pub static FILES: Mutex<Vec<(String, Vec<u8>)>> = Mutex::new(Vec::new());

    pub fn fetch(url: &str) -> Result<super::Response, String> {
        let files = FILES.lock().unwrap();
        match files.iter().find(|(u, _)| u == url) {
            Some((_, body)) => Ok(super::Response {
                status: 200,
                body: body.clone(),
                retry_after: None,
            }),
            None => Ok(super::Response {
                status: 404,
                ..super::Response::default()
            }),
        }
    }
}
