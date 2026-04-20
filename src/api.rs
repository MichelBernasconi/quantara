use axum::{
    extract::{State, Path},
    routing::{get, post},
    Json, Router,
};
use tower_http::cors::{Any, CorsLayer};
use crate::domain::*;
use crate::engine::BacktestEngine;
use crate::metrics::{calculate_metrics, BacktestMetrics};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use uuid::Uuid;
use rust_decimal_macros::dec;

pub struct AppState {
    pub assets: RwLock<HashMap<Uuid, Asset>>,
    pub data: RwLock<HashMap<Uuid, TimeSeries>>,
    pub strategies: RwLock<HashMap<Uuid, Strategy>>,
}

pub fn create_router() -> Router {
    let state = Arc::new(AppState {
        assets: RwLock::new(HashMap::new()),
        data: RwLock::new(HashMap::new()),
        strategies: RwLock::new(HashMap::new()),
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/assets", post(create_asset).get(list_assets))
        .route("/data", post(upload_data))
        .route("/strategies", post(create_strategy).get(list_strategies))
        .route("/backtest/:strategy_id", post(run_backtest))
        .layer(cors)
        .with_state(state)
}

async fn create_asset(
    State(state): State<Arc<AppState>>,
    Json(asset_req): Json<Asset>,
) -> Json<Asset> {
    let mut assets = state.assets.write().unwrap();
    assets.insert(asset_req.id, asset_req.clone());
    Json(asset_req)
}

async fn list_assets(State(state): State<Arc<AppState>>) -> Json<Vec<Asset>> {
    let assets = state.assets.read().unwrap();
    Json(assets.values().cloned().collect())
}

async fn upload_data(
    State(state): State<Arc<AppState>>,
    Json(ts): Json<TimeSeries>,
) -> Json<String> {
    let mut data = state.data.write().unwrap();
    data.insert(ts.asset_id, ts);
    Json("Data uploaded successfully".to_string())
}

async fn create_strategy(
    State(state): State<Arc<AppState>>,
    Json(mut strategy): Json<Strategy>,
) -> Json<Strategy> {
    if strategy.id == Uuid::nil() {
        strategy.id = Uuid::new_v4();
    }
    let mut strategies = state.strategies.write().unwrap();
    strategies.insert(strategy.id, strategy.clone());
    Json(strategy)
}

async fn list_strategies(State(state): State<Arc<AppState>>) -> Json<Vec<Strategy>> {
    let strategies = state.strategies.read().unwrap();
    Json(strategies.values().cloned().collect())
}

async fn run_backtest(
    State(state): State<Arc<AppState>>,
    Path(strategy_id): Path<Uuid>,
) -> Result<Json<BacktestMetrics>, String> {
    let strategy = {
        let strategies = state.strategies.read().unwrap();
        strategies.get(&strategy_id).cloned().ok_or("Strategy not found")?
    };

    let data = state.data.read().unwrap();
    
    let mut engine = BacktestEngine::new(dec!(100000)); // Start with 100k
    engine.run(&strategy, &data).map_err(|e| e.to_string())?;

    let metrics = calculate_metrics(&engine.history);
    Ok(Json(metrics))
}
