use std::{convert::Infallible, io, time::Duration};

use actix_web::{get, middleware::Logger, App, HttpServer, Responder};
use actix_web_lab::{extract::Path, respond::Html, sse};
use futures_util::stream;
use time::format_description::well_known::Rfc3339;
use tokio::time::sleep;

#[get("/")]
async fn index() -> impl Responder {
    Html(include_str!("./assets/sse.html").to_string())
}

#[get("/countdown")]
async fn countdown() -> impl Responder {
    common_countdown(8)
}

#[get("/countdown/{n:\\d+}")]
async fn countdown_from(Path((n,)): Path<(u32,)>) -> impl Responder {
    common_countdown(n.try_into().unwrap())
}

fn common_countdown(n: i32) -> impl Responder {
    let countdown_stream = stream::unfold((false, n), |(started, n)| async move {
        // allow first countdown value to yield immediately
        if started {
            sleep(Duration::from_secs(1)).await;
        }

        if n > 0 {
            let msg = sse::Event::Data(sse::Data::new(n.to_string()).event("countdown"));
            Some((Ok::<_, Infallible>(msg), (true, n - 1)))
        } else {
            None
        }
    });

    sse::Sse::from_stream(countdown_stream).with_retry_duration(Duration::from_secs(5))
}

#[get("/time")]
async fn timestamp() -> impl Responder {
    let (sender, sse) = sse::channel(2);

    actix_web::rt::spawn(async move {
        loop {
            let time = time::OffsetDateTime::now_utc();
            let msg = sse::Data::new(time.format(&Rfc3339).unwrap()).event("timestamp");

            if sender.send(msg).await.is_err() {
                tracing::warn!("client disconnected; could not send SSE message");
                break;
            }

            sleep(Duration::from_secs(10)).await;
        }
    });

    sse.with_keep_alive(Duration::from_secs(3))
}

#[actix_web::main]
async fn main() -> io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    tracing::info!("starting HTTP server at http://localhost:8080");

    HttpServer::new(|| {
        App::new()
            .service(index)
            .service(countdown)
            .service(countdown_from)
            .service(timestamp)
            .wrap(Logger::default())
    })
    .workers(2)
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
