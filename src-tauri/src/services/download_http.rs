use reqwest::{header, Client, Response, StatusCode};
use std::time::Duration;
use tokio::sync::oneshot;

const MAX_ATTEMPTS: u32 = 3;

pub fn client() -> Result<Client, String> {
    Client::builder()
        .connect_timeout(Duration::from_secs(20))
        .read_timeout(Duration::from_secs(45))
        .build()
        .map_err(|e| e.to_string())
}

fn retryable_status(status: StatusCode) -> bool {
    matches!(status.as_u16(), 408 | 429 | 500 | 502 | 503 | 504)
}

pub fn transport_error(error: reqwest::Error) -> String {
    let kind = if error.is_timeout() {
        "网络超时"
    } else if error.is_connect() {
        "网络连接失败"
    } else {
        "网络传输失败"
    };
    let error = error.without_url();
    let mut details = vec![error.to_string()];
    let mut source = std::error::Error::source(&error);
    while let Some(cause) = source {
        details.push(cause.to_string());
        source = cause.source();
    }
    format!(
        "{}: {}。已保留下载进度，可稍后重试。",
        kind,
        details.join(" → ")
    )
}

/// Retry only temporary transport/server failures, with cancellable bounded backoff.
pub async fn request(
    client: &Client,
    url: &str,
    cookie: &str,
    offset: u64,
    cancel: &mut oneshot::Receiver<()>,
    mut on_retry: impl FnMut(u32, u64),
) -> Result<Response, String> {
    request_with_delay(
        client,
        url,
        cookie,
        offset,
        cancel,
        &mut on_retry,
        Duration::from_secs(1),
    )
    .await
}

async fn request_with_delay(
    client: &Client,
    url: &str,
    cookie: &str,
    offset: u64,
    cancel: &mut oneshot::Receiver<()>,
    on_retry: &mut impl FnMut(u32, u64),
    base_delay: Duration,
) -> Result<Response, String> {
    let url = reqwest::Url::parse(url).map_err(|e| format!("无效下载地址: {}", e))?;
    for attempt in 0..MAX_ATTEMPTS {
        let mut req=client.get(url.clone())
            .header(header::USER_AGENT,"Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/120.0.0.0 Safari/537.36")
            .header(header::ACCEPT,"application/octet-stream,application/zip,*/*")
            .header(header::ACCEPT_ENCODING,"identity")
            .header(header::REFERER,"https://hub.virtamate.com/");
        // Explicit CDN requests must not receive the user's Hub session cookie.
        if url.host_str() == Some("hub.virtamate.com") {
            req = req.header(header::COOKIE, cookie);
        }
        if offset > 0 {
            req = req.header(header::RANGE, format!("bytes={}-", offset));
        }
        let response = tokio::select! {
            biased;
            _=&mut *cancel=>return Err("下载已暂停或取消".into()),
            result=tokio::time::timeout(Duration::from_secs(45),req.send())=>result,
        };
        let mut delay = base_delay * (1 << attempt);
        let error = match response {
            Ok(Ok(response)) => {
                let status = response.status();
                if status.is_success() || status == StatusCode::RANGE_NOT_SATISFIABLE {
                    return Ok(response);
                }
                if !retryable_status(status) {
                    return Err(match status.as_u16() {
                        401 | 403 => format!(
                            "在线库拒绝访问（HTTP {}），请检查 Hub 登录状态及该资源的下载权限。",
                            status.as_u16()
                        ),
                        404 => "下载地址不存在或已失效（HTTP 404），请重新从在线库获取下载地址。"
                            .into(),
                        _ => format!("在线库返回 HTTP {}，已保留下载进度。", status.as_u16()),
                    });
                }
                if let Some(seconds) = response
                    .headers()
                    .get(header::RETRY_AFTER)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                {
                    // Do not retry sooner than the server requested.
                    if seconds > 60 {
                        return Err(format!(
                            "在线库限流或暂不可用（HTTP {}），请 {} 秒后重试；下载进度已保留。",
                            status.as_u16(),
                            seconds
                        ));
                    }
                    delay = Duration::from_secs(seconds);
                }
                format!(
                    "在线库暂不可用（HTTP {}），已尝试 {} 次；下载进度已保留。",
                    status.as_u16(),
                    attempt + 1
                )
            }
            Ok(Err(error)) => {
                if !error.is_connect() && !error.is_timeout() && !error.is_request() {
                    return Err(transport_error(error));
                }
                transport_error(error)
            }
            Err(_) => "请求响应超时，下载进度已保留。".into(),
        };
        if attempt + 1 == MAX_ATTEMPTS {
            return Err(error);
        }
        on_retry(attempt + 2, delay.as_secs());
        tokio::select! { biased; _=&mut *cancel=>return Err("下载已暂停或取消".into()), _=tokio::time::sleep(delay)=>{} }
    }
    unreachable!()
}

/// The offset must match before any data is appended. A 200 response starts over.
pub fn response_layout(
    status: StatusCode,
    headers: &header::HeaderMap,
    offset: u64,
) -> Result<(bool, u64), String> {
    let length = headers
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());
    if status == StatusCode::OK {
        return Ok((offset > 0, length.unwrap_or(0)));
    }
    if status != StatusCode::PARTIAL_CONTENT {
        return Err(format!("不是可下载的文件响应: HTTP {}", status.as_u16()));
    }
    let range = headers
        .get(header::CONTENT_RANGE)
        .and_then(|v| v.to_str().ok())
        .ok_or("续传响应缺少 Content-Range，已保留临时文件")?;
    let parsed = (|| {
        let (span, total) = range.strip_prefix("bytes ")?.split_once('/')?;
        let (start, end) = span.split_once('-')?;
        Some((
            start.parse::<u64>().ok()?,
            end.parse::<u64>().ok()?,
            total.parse::<u64>().ok()?,
        ))
    })()
    .ok_or("续传响应范围格式无效，已保留临时文件")?;
    let (start, end, total) = parsed;
    if start != offset
        || end < start
        || end.checked_add(1) != Some(total)
        || length.is_some_and(|n| n != end - start + 1)
    {
        return Err("续传响应范围与本地进度不符，未拼接数据；请重新获取下载地址。".into());
    }
    Ok((false, total))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    fn server(responses: Vec<&'static str>) -> (String, std::thread::JoinHandle<Vec<String>>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/file", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let mut requests = vec![];
            for response in responses {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut bytes = vec![];
                let mut buf = [0; 1024];
                while !bytes.ends_with(b"\r\n\r\n") {
                    let n = stream.read(&mut buf).unwrap();
                    if n == 0 {
                        break;
                    }
                    bytes.extend_from_slice(&buf[..n]);
                }
                requests.push(String::from_utf8_lossy(&bytes).into_owned());
                let _ = stream.write_all(response.as_bytes());
            }
            requests
        });
        (url, handle)
    }
    #[tokio::test]
    async fn retries_temporary_status_and_keeps_range_without_leaking_cookie() {
        let (url,server)=server(vec!["HTTP/1.1 503 Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n","HTTP/1.1 206 Partial Content\r\nContent-Length: 3\r\nContent-Range: bytes 2-4/5\r\nConnection: close\r\n\r\nabc"]);
        let (_tx, mut rx) = oneshot::channel();
        let mut attempts = vec![];
        let res = request_with_delay(
            &Client::builder().no_proxy().build().unwrap(),
            &url,
            "secret-cookie",
            2,
            &mut rx,
            &mut |n, _| attempts.push(n),
            Duration::from_millis(1),
        )
        .await
        .unwrap();
        assert_eq!(
            response_layout(res.status(), res.headers(), 2).unwrap(),
            (false, 5)
        );
        assert_eq!(res.text().await.unwrap(), "abc");
        assert_eq!(attempts, vec![2]);
        let requests = server.join().unwrap();
        assert_eq!(requests.len(), 2);
        assert!(requests
            .iter()
            .all(|s| s.to_lowercase().contains("range: bytes=2-")));
        assert!(requests
            .iter()
            .all(|s| !s.to_lowercase().contains("cookie:")));
    }
    #[tokio::test]
    async fn permission_error_is_not_retried() {
        let (url, server) = server(vec![
            "HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        ]);
        let (_tx, mut rx) = oneshot::channel();
        let error = request(
            &Client::builder().no_proxy().build().unwrap(),
            &url,
            "",
            0,
            &mut rx,
            |_, _| panic!("must not retry"),
        )
        .await
        .unwrap_err();
        assert!(error.contains("权限"));
        assert_eq!(server.join().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn retry_count_is_bounded() {
        let (url, server) = server(
            vec!["HTTP/1.1 503 Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"; 3],
        );
        let (_tx, mut rx) = oneshot::channel();
        let mut retries = vec![];
        let error = request_with_delay(
            &Client::builder().no_proxy().build().unwrap(),
            &url,
            "",
            0,
            &mut rx,
            &mut |attempt, _| retries.push(attempt),
            Duration::from_millis(1),
        )
        .await
        .unwrap_err();
        assert!(error.contains("3 次"));
        assert_eq!(retries, vec![2, 3]);
        assert_eq!(server.join().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn cancelled_backoff_preserves_responsiveness() {
        let (url,server)=server(vec!["HTTP/1.1 429 Too Many Requests\r\nRetry-After: 60\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"]);
        let (tx, mut rx) = oneshot::channel();
        let mut tx = Some(tx);
        let result = tokio::time::timeout(
            Duration::from_secs(2),
            request(
                &Client::builder().no_proxy().build().unwrap(),
                &url,
                "",
                0,
                &mut rx,
                |_, _| {
                    tx.take().unwrap().send(()).unwrap();
                },
            ),
        )
        .await
        .unwrap();
        assert!(result.unwrap_err().contains("取消"));
        assert_eq!(server.join().unwrap().len(), 1);
    }
    #[tokio::test]
    async fn cancelled_request_does_not_wait_for_connection() {
        let (tx, mut rx) = oneshot::channel();
        tx.send(()).unwrap();
        assert!(request(
            &Client::new(),
            "http://127.0.0.1:1/file",
            "",
            0,
            &mut rx,
            |_, _| {}
        )
        .await
        .unwrap_err()
        .contains("取消"));
    }
    #[test]
    fn rejects_wrong_offset_and_restarts_when_range_ignored() {
        let mut headers = header::HeaderMap::new();
        headers.insert(header::CONTENT_RANGE, "bytes 0-4/5".parse().unwrap());
        headers.insert(header::CONTENT_LENGTH, "5".parse().unwrap());
        assert!(response_layout(StatusCode::PARTIAL_CONTENT, &headers, 2).is_err());
        assert_eq!(
            response_layout(StatusCode::OK, &headers, 2).unwrap(),
            (true, 5)
        );
        headers.remove(header::CONTENT_RANGE);
        assert!(response_layout(StatusCode::PARTIAL_CONTENT, &headers, 2).is_err());
    }
}
