/// Web Server Binary: Serves the M Engine playground
/// Usage: cargo run --bin web

use actix_web::{web, App, HttpServer, middleware};
use actix_cors::Cors;
use std::path::PathBuf;
use m_engine::api::{health, compile, compile_with_debug};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .try_init()
        .ok();
    
    let addr = "127.0.0.1:8080";
    println!("🚀 M Engine Playground");
    println!("📍 Server: http://{}", addr);
    println!("📝 Editor: http://{}/", addr);
    println!("\nPress Ctrl+C to stop...\n");
    
    HttpServer::new(|| {
        let cors = Cors::permissive();
        
        App::new()
            .wrap(cors)
            .wrap(middleware::Logger::default())
            // API routes
            .route("/health", web::get().to(health))
            .route("/api/compile", web::post().to(compile))
            .route("/api/compile/debug", web::post().to(compile_with_debug))
            // Serve static files (HTML, CSS, JS)
            .service(actix_files::Files::new(
                "/",
                find_public_dir(),
            ).index_file("index.html"))
    })
    .bind(addr)?
    .run()
    .await
}

fn find_public_dir() -> PathBuf {
    // Try different paths depending on where we're running from
    let candidates = vec![
        "public",
        "src/../public",
        "../../public",
    ];
    
    for path in candidates {
        let p = PathBuf::from(path);
        if p.exists() {
            return p;
        }
    }
    
    // Default fallback
    PathBuf::from("public")
}
