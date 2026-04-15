use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use uuid::Uuid;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AssetType {
    Stock,
    Bond,
    ETF,
    Crypto,
    Commodity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    pub id: Uuid,
    pub symbol: String,
    pub name: String,
    pub asset_type: AssetType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricePoint {
    pub timestamp: DateTime<Utc>,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeries {
    pub asset_id: Uuid,
    pub data: Vec<PricePoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuleOperator {
    GreaterThan,
    LessThan,
    Equal,
    CrossesOver,
    CrossesUnder,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Indicator {
    SMA(usize), // Simple Moving Average period
    EMA(usize), // Exponential Moving Average period
    RSI(usize), // Relative Strength Index period
    Price,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub left: Indicator,
    pub operator: RuleOperator,
    pub right: Indicator,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Strategy {
    pub id: Uuid,
    pub name: String,
    pub entry_rules: Vec<Rule>,
    pub exit_rules: Vec<Rule>,
    pub rebalance_interval_days: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: Uuid,
    pub asset_id: Uuid,
    pub quantity: Decimal,
    pub price: Decimal,
    pub timestamp: DateTime<Utc>,
    pub side: OrderSide,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub asset_id: Uuid,
    pub quantity: Decimal,
    pub average_price: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Portfolio {
    pub cash: Decimal,
    pub positions: HashMap<Uuid, Position>,
}
