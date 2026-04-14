/// Web API handlers using actix-web
use actix_web::{web, HttpResponse, Result};
use serde_json::json;
use crate::{CompileRequest, compile_formula};

pub async fn health() -> Result<HttpResponse> {
    Ok(HttpResponse::Ok().json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "uptime": "running"
    })))
}

pub async fn compile(req: web::Json<CompileRequest>) -> Result<HttpResponse> {
    let response = compile_formula(req.into_inner());
    Ok(HttpResponse::Ok().json(response))
}

pub async fn compile_with_debug(req: web::Json<CompileRequest>) -> Result<HttpResponse> {
    let mut req = req.into_inner();
    req.debug = Some(true);
    let response = compile_formula(req);
    Ok(HttpResponse::Ok().json(response))
}
