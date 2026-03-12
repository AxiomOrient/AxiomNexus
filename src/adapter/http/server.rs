use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    sync::mpsc::Receiver,
    thread,
};

use crate::adapter::{
    http::dto::EVENTS_ROUTE,
    sse::{encode_sse_event, EmittedSseEvent, SseBroker},
};

use super::transport::{HttpRequest, HttpResponse, HttpTransport};

pub(crate) fn serve<S>(transport: HttpTransport<S>, bind_addr: &str) -> Result<(), std::io::Error>
where
    S: crate::port::store::CommandStorePort + crate::port::store::QueryStorePort,
{
    let listener = TcpListener::bind(bind_addr)?;
    serve_listener(listener, transport, None)
}

fn serve_listener<S>(
    listener: TcpListener,
    transport: HttpTransport<S>,
    max_requests: Option<usize>,
) -> Result<(), std::io::Error>
where
    S: crate::port::store::CommandStorePort + crate::port::store::QueryStorePort,
{
    let broker = SseBroker::default();

    for (served, stream) in listener.incoming().enumerate() {
        let mut stream = stream?;
        match read_request(&mut stream) {
            Ok(Some(request)) if is_sse_subscribe(&request) => {
                let events = broker.subscribe();
                thread::spawn(move || {
                    let _ = write_sse_response(stream, events);
                });
            }
            Ok(Some(request)) => {
                let response = transport.handle(request);
                let emitted_event = response.emitted_event.clone();
                write_response(&mut stream, response)?;
                if let Some(event) = emitted_event {
                    broker.publish(event);
                }
            }
            Ok(None) => {}
            Err(error) => {
                write_response(
                    &mut stream,
                    HttpResponse {
                        status: 400,
                        body: format!(r#"{{"error":"invalid request: {error}"}}"#),
                        emitted_event: None,
                    },
                )?;
            }
        }
        if max_requests.is_some_and(|limit| served + 1 >= limit) {
            break;
        }
    }

    Ok(())
}

fn is_sse_subscribe(request: &HttpRequest) -> bool {
    request.method == "GET" && request.path == EVENTS_ROUTE
}

fn read_request(stream: &mut TcpStream) -> Result<Option<HttpRequest>, std::io::Error> {
    let mut reader = BufReader::new(stream);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line)? == 0 {
        return Ok(None);
    }
    if request_line.trim().is_empty() {
        return Ok(None);
    }

    let mut parts = request_line.split_whitespace();
    let method = match parts.next() {
        Some("GET") => "GET",
        Some("POST") => "POST",
        Some(other) => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("unsupported method `{other}`"),
            ))
        }
        None => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "missing request method",
            ))
        }
    };
    let path = parts
        .next()
        .ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "missing request path")
        })?
        .split('?')
        .next()
        .unwrap_or("/")
        .to_owned();

    let mut content_length = 0usize;
    loop {
        let mut header = String::new();
        reader.read_line(&mut header)?;
        if header == "\r\n" || header.is_empty() {
            break;
        }
        if let Some(value) = header.strip_prefix("Content-Length:") {
            content_length = value.trim().parse().map_err(|error| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("invalid content length: {error}"),
                )
            })?;
        }
    }

    let body = if content_length == 0 {
        None
    } else {
        let mut bytes = vec![0; content_length];
        reader.read_exact(&mut bytes)?;
        Some(String::from_utf8(bytes).map_err(|error| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("request body is not valid utf-8: {error}"),
            )
        })?)
    };

    Ok(Some(HttpRequest { method, path, body }))
}

fn write_response(stream: &mut TcpStream, response: HttpResponse) -> Result<(), std::io::Error> {
    let reason = match response.status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        404 => "Not Found",
        409 => "Conflict",
        503 => "Service Unavailable",
        _ => "Internal Server Error",
    };
    write!(
        stream,
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        response.status,
        reason,
        response.body.len(),
        response.body
    )?;
    stream.flush()?;
    Ok(())
}

fn write_sse_response(
    mut stream: TcpStream,
    events: Receiver<EmittedSseEvent>,
) -> Result<(), std::io::Error> {
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n"
    )?;
    stream.flush()?;

    for event in events {
        stream.write_all(encode_sse_event(&event).as_bytes())?;
        stream.flush()?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        thread,
        time::Duration,
    };

    use crate::adapter::memory::store::MemoryStore;

    use super::serve_listener;
    use crate::{
        adapter::http::{dto::WORK_WAKE_ROUTE, transport::HttpTransport},
        adapter::memory::store::DEMO_TODO_WORK_ID,
    };

    #[test]
    fn serve_handles_real_post_then_get_flow() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
        let addr = listener.local_addr().expect("local addr should exist");
        let body = "{\"name\":\"TCP Co\",\"description\":\"tcp flow\"}";

        let server = thread::spawn(move || {
            let transport = HttpTransport::new(MemoryStore::demo());
            serve_listener(listener, transport, Some(2)).expect("server should serve two requests");
        });

        let create_response = send_request(
            addr,
            &format!(
                "POST /api/companies HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            ),
        );
        assert!(create_response.starts_with("HTTP/1.1 200 OK"));
        assert!(create_response.contains("\"company_id\":\"00000000-0000-4000-8000-"));
        assert!(!create_response.contains("0x"));

        let list_response = send_request(
            addr,
            "GET /api/companies HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\n\r\n",
        );
        assert!(list_response.starts_with("HTTP/1.1 200 OK"));
        assert!(list_response.contains("\"name\":\"TCP Co\""));
        assert!(!list_response.contains("0x000000000"));

        server.join().expect("server should join");
    }

    #[test]
    fn serve_returns_bad_request_for_unsupported_method() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
        let addr = listener.local_addr().expect("local addr should exist");

        let server = thread::spawn(move || {
            let transport = HttpTransport::new(MemoryStore::demo());
            serve_listener(listener, transport, Some(1)).expect("server should serve one request");
        });

        let response = send_request(
            addr,
            "PUT /api/board HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\n\r\n",
        );

        assert!(response.starts_with("HTTP/1.1 400 Bad Request"));
        assert!(response.contains("unsupported method"));

        server.join().expect("server should join");
    }

    #[test]
    fn serve_streams_after_commit_events_over_sse() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
        let addr = listener.local_addr().expect("local addr should exist");

        let server = thread::spawn(move || {
            let transport = HttpTransport::new(MemoryStore::demo());
            serve_listener(listener, transport, Some(2)).expect("server should serve sse + wake");
        });

        let sse_reader = thread::spawn(move || {
            let mut stream = TcpStream::connect(addr).expect("sse client should connect");
            write!(
                stream,
                "GET /api/events HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\n\r\n"
            )
            .expect("sse request should write");
            stream
                .shutdown(std::net::Shutdown::Write)
                .expect("sse write half should close");
            let mut response = String::new();
            stream
                .read_to_string(&mut response)
                .expect("sse response should read");
            response
        });

        thread::sleep(Duration::from_millis(50));
        let wake_response = send_request(
            addr,
            &format!(
                "POST {} HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                WORK_WAKE_ROUTE.replace("{id}", DEMO_TODO_WORK_ID),
                "{\"latest_reason\":\"follow up\",\"obligation_delta\":[\"cargo test\"]}".len(),
                "{\"latest_reason\":\"follow up\",\"obligation_delta\":[\"cargo test\"]}"
            ),
        );

        let sse_response = sse_reader.join().expect("sse client should join");

        assert!(wake_response.starts_with("HTTP/1.1 202 Accepted"));
        assert!(sse_response.starts_with("HTTP/1.1 200 OK"));
        assert!(sse_response.contains("Content-Type: text/event-stream"));
        assert!(sse_response.contains("\"data\":\"wake merged 1\""));

        server.join().expect("server should join");
    }

    fn send_request(addr: std::net::SocketAddr, request: &str) -> String {
        let mut stream = loop {
            match TcpStream::connect(addr) {
                Ok(stream) => break stream,
                Err(_) => thread::yield_now(),
            }
        };
        write!(stream, "{request}").expect("request should write");
        stream
            .shutdown(std::net::Shutdown::Write)
            .expect("write half should close");

        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .expect("response should read");
        response
    }
}
