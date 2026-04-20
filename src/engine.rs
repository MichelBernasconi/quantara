use crate::domain::*;
use anyhow::Result;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::HashMap;
use uuid::Uuid;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyModule};

pub struct BacktestEngine {
    pub portfolio: Portfolio,
    pub history: Vec<PortfolioSnapshot>,
    pub trades: Vec<Order>,
    pub last_rebalance: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone)]
pub struct PortfolioSnapshot {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub total_value: Decimal,
    pub cash: Decimal,
    pub holdings_value: Decimal,
}

impl BacktestEngine {
    pub fn new(initial_cash: Decimal) -> Self {
        Self {
            portfolio: Portfolio {
                cash: initial_cash,
                positions: HashMap::new(),
            },
            history: Vec::new(),
            trades: Vec::new(),
            last_rebalance: None,
        }
    }

    pub fn run(
        &mut self,
        strategy: &Strategy,
        time_series_map: &HashMap<Uuid, TimeSeries>,
    ) -> Result<()> {
        let mut timestamps: Vec<_> = time_series_map
            .values()
            .flat_map(|ts| ts.data.iter().map(|p| p.timestamp))
            .collect();
        timestamps.sort();
        timestamps.dedup();

        for ts in timestamps {
            self.step(ts, strategy, time_series_map)?;
        }

        Ok(())
    }

    fn step(
        &mut self,
        timestamp: chrono::DateTime<chrono::Utc>,
        strategy: &Strategy,
        time_series_map: &HashMap<Uuid, TimeSeries>,
    ) -> Result<()> {
        // 1. Update current valuation
        let mut holdings_value = dec!(0);
        for (asset_id, pos) in &self.portfolio.positions {
            if let Some(price) = self.get_price_at(*asset_id, timestamp, time_series_map) {
                holdings_value += pos.quantity * price;
            }
        }

        self.history.push(PortfolioSnapshot {
            timestamp,
            total_value: self.portfolio.cash + holdings_value,
            cash: self.portfolio.cash,
            holdings_value,
        });

        // 2. Logic Selection: Rule-Based or Python
        match &strategy.strategy_type {
            StrategyType::RuleBased { entry_rules, exit_rules } => {
                self.process_rule_based(timestamp, entry_rules, exit_rules, time_series_map)?;
            }
            StrategyType::Python { script } => {
                self.process_python(timestamp, script, time_series_map)?;
            }
        }

        // 3. Handle Rebalancing (Only for rule-based or generic rebalance logic)
        if let Some(interval) = strategy.rebalance_interval_days {
            let should_rebalance = match self.last_rebalance {
                None => true,
                Some(last) => (timestamp - last).num_days() >= interval,
            };

            if should_rebalance {
                self.rebalance(timestamp, time_series_map)?;
                self.last_rebalance = Some(timestamp);
            }
        }

        Ok(())
    }

    fn process_rule_based(
        &mut self,
        timestamp: chrono::DateTime<chrono::Utc>,
        entry_rules: &[Rule],
        exit_rules: &[Rule],
        time_series_map: &HashMap<Uuid, TimeSeries>,
    ) -> Result<()> {
        let mut to_sell = Vec::new();
        for asset_id in self.portfolio.positions.keys() {
            if self.should_exit_rules(*asset_id, timestamp, exit_rules, time_series_map) {
                to_sell.push(*asset_id);
            }
        }
        for asset_id in to_sell {
            self.execute_sell(asset_id, timestamp, time_series_map)?;
        }

        for asset_id in time_series_map.keys() {
            if !self.portfolio.positions.contains_key(asset_id) {
                if self.should_enter_rules(*asset_id, timestamp, entry_rules, time_series_map) {
                    self.execute_buy(*asset_id, timestamp, time_series_map)?;
                }
            }
        }
        Ok(())
    }

    fn process_python(
        &mut self,
        timestamp: chrono::DateTime<chrono::Utc>,
        script: &str,
        time_series_map: &HashMap<Uuid, TimeSeries>,
    ) -> Result<()> {
        Python::with_gil(|py| {
            // Setup the execution context
            let locals = PyDict::new_bound(py);
            
            // Inject Portfolio data
            let portfolio_dict = PyDict::new_bound(py);
            portfolio_dict.set_item("cash", self.portfolio.cash.to_string())?;
            
            let positions_dict = PyDict::new_bound(py);
            for (id, pos) in &self.portfolio.positions {
                let pos_info = PyDict::new_bound(py);
                pos_info.set_item("quantity", pos.quantity.to_string())?;
                pos_info.set_item("avg_price", pos.average_price.to_string())?;
                positions_dict.set_item(id.to_string(), pos_info)?;
            }
            portfolio_dict.set_item("positions", positions_dict)?;
            locals.set_item("portfolio", portfolio_dict)?;

            // Inject Market Data for current timestamp
            let market_dict = PyDict::new_bound(py);
            for (id, ts) in time_series_map {
                if let Some(point) = ts.data.iter().find(|p| p.timestamp == timestamp) {
                    let point_dict = PyDict::new_bound(py);
                    point_dict.set_item("price", point.close.to_string())?;
                    point_dict.set_item("volume", point.volume.map(|v| v.to_string()))?;
                    market_dict.set_item(id.to_string(), point_dict)?;
                }
            }
            locals.set_item("market", market_dict)?;
            locals.set_item("timestamp", timestamp.to_rfc3339())?;

            // Execute the user script
            py.run_bound(script, None, Some(&locals))?;

            // Look for signals in the script output (expected a variable named 'signal')
            if let Ok(signal) = locals.get_item("signal") {
                if let Some(signal_str) = signal {
                    let signal_val: String = signal_str.extract()?;
                    self.handle_python_signal(&signal_val, timestamp, time_series_map)?;
                }
            }
            
            Ok::<(), PyErr>(())
        }).map_err(|e| anyhow::anyhow!("Python Error: {}", e))?;

        Ok(())
    }

    fn handle_python_signal(
        &mut self,
        signal: &str,
        timestamp: chrono::DateTime<chrono::Utc>,
        time_series_map: &HashMap<Uuid, TimeSeries>
    ) -> Result<()> {
        // Format of signal expected: "BUY:asset_uuid" or "SELL:asset_uuid"
        let parts: Vec<&str> = signal.split(':').collect();
        if parts.len() < 2 { return Ok(()); }

        let action = parts[0];
        if let Ok(asset_id) = Uuid::parse_str(parts[1]) {
            match action {
                "BUY" => {
                    if !self.portfolio.positions.contains_key(&asset_id) {
                        self.execute_buy(asset_id, timestamp, time_series_map)?;
                    }
                }
                "SELL" => {
                    if self.portfolio.positions.contains_key(&asset_id) {
                        self.execute_sell(asset_id, timestamp, time_series_map)?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn get_price_at(
        &self,
        asset_id: Uuid,
        timestamp: chrono::DateTime<chrono::Utc>,
        time_series_map: &HashMap<Uuid, TimeSeries>,
    ) -> Option<Decimal> {
        time_series_map.get(&asset_id).and_then(|ts| {
            ts.data.iter().find(|p| p.timestamp == timestamp).map(|p| p.close)
        })
    }

    fn should_enter_rules(
        &self,
        asset_id: Uuid,
        timestamp: chrono::DateTime<chrono::Utc>,
        entry_rules: &[Rule],
        time_series_map: &HashMap<Uuid, TimeSeries>,
    ) -> bool {
        if entry_rules.is_empty() { return false; }
        entry_rules.iter().all(|rule| {
            self.evaluate_rule(rule, asset_id, timestamp, time_series_map)
        })
    }

    fn should_exit_rules(
        &self,
        asset_id: Uuid,
        timestamp: chrono::DateTime<chrono::Utc>,
        exit_rules: &[Rule],
        time_series_map: &HashMap<Uuid, TimeSeries>,
    ) -> bool {
        if exit_rules.is_empty() { return false; }
        exit_rules.iter().any(|rule| {
            self.evaluate_rule(rule, asset_id, timestamp, time_series_map)
        })
    }

    fn evaluate_rule(
        &self,
        rule: &Rule,
        asset_id: Uuid,
        timestamp: chrono::DateTime<chrono::Utc>,
        time_series_map: &HashMap<Uuid, TimeSeries>,
    ) -> bool {
        let left_val = self.calculate_indicator(&rule.left, asset_id, timestamp, time_series_map);
        let right_val = self.calculate_indicator(&rule.right, asset_id, timestamp, time_series_map);

        match (left_val, right_val) {
            (Some(l), Some(r)) => match rule.operator {
                RuleOperator::GreaterThan => l > r,
                RuleOperator::LessThan => l < r,
                RuleOperator::Equal => l == r,
                RuleOperator::CrossesOver => true, // Simplification
                RuleOperator::CrossesUnder => true,
            },
            _ => false,
        }
    }

    fn calculate_indicator(
        &self,
        indicator: &Indicator,
        asset_id: Uuid,
        timestamp: chrono::DateTime<chrono::Utc>,
        time_series_map: &HashMap<Uuid, TimeSeries>,
    ) -> Option<Decimal> {
        match indicator {
            Indicator::Price => self.get_price_at(asset_id, timestamp, time_series_map),
            Indicator::SMA(period) => self.calculate_sma(asset_id, timestamp, *period, time_series_map),
            Indicator::RSI(period) => self.calculate_rsi(asset_id, timestamp, *period, time_series_map),
            Indicator::Value(val) => Some(*val),
            Indicator::EMA(period) => self.calculate_sma(asset_id, timestamp, *period, time_series_map), // Placeholder
        }
    }

    fn calculate_sma(
        &self,
        asset_id: Uuid,
        timestamp: chrono::DateTime<chrono::Utc>,
        period: usize,
        time_series_map: &HashMap<Uuid, TimeSeries>,
    ) -> Option<Decimal> {
        let ts = time_series_map.get(&asset_id)?;
        let data: Vec<_> = ts.data.iter()
            .filter(|p| p.timestamp <= timestamp)
            .rev()
            .take(period)
            .collect();
        
        if data.len() < period {
            return None;
        }

        let sum: Decimal = data.iter().map(|p| p.close).sum();
        Some(sum / Decimal::from(period))
    }

    fn execute_buy(
        &mut self,
        asset_id: Uuid,
        timestamp: chrono::DateTime<chrono::Utc>,
        time_series_map: &HashMap<Uuid, TimeSeries>,
    ) -> Result<()> {
        if let Some(price) = self.get_price_at(asset_id, timestamp, time_series_map) {
            let amount_to_spend = self.portfolio.cash;
            if amount_to_spend > dec!(0) {
                let quantity = amount_to_spend / price;
                self.portfolio.cash -= amount_to_spend;
                
                let entry = self.portfolio.positions.entry(asset_id).or_insert(Position {
                    asset_id,
                    quantity: dec!(0),
                    average_price: dec!(0),
                });
                
                entry.average_price = (entry.average_price * entry.quantity + price * quantity) / (entry.quantity + quantity);
                entry.quantity += quantity;

                self.trades.push(Order {
                    id: Uuid::new_v4(),
                    asset_id,
                    quantity,
                    price,
                    timestamp,
                    side: OrderSide::Buy,
                });
            }
        }
        Ok(())
    }

    fn execute_sell(
        &mut self,
        asset_id: Uuid,
        timestamp: chrono::DateTime<chrono::Utc>,
        time_series_map: &HashMap<Uuid, TimeSeries>,
    ) -> Result<()> {
        if let Some(pos) = self.portfolio.positions.remove(&asset_id) {
            if let Some(price) = self.get_price_at(asset_id, timestamp, time_series_map) {
                let proceeds = pos.quantity * price;
                self.portfolio.cash += proceeds;

                self.trades.push(Order {
                    id: Uuid::new_v4(),
                    asset_id,
                    quantity: pos.quantity,
                    price,
                    timestamp,
                    side: OrderSide::Sell,
                });
            }
        }
        Ok(())
    }

    fn calculate_rsi(
        &self,
        asset_id: Uuid,
        timestamp: chrono::DateTime<chrono::Utc>,
        period: usize,
        time_series_map: &HashMap<Uuid, TimeSeries>,
    ) -> Option<Decimal> {
        let ts = time_series_map.get(&asset_id)?;
        let data: Vec<_> = ts.data.iter()
            .filter(|p| p.timestamp <= timestamp)
            .rev()
            .take(period + 1)
            .collect();
        
        if data.len() < period + 1 { return None; }

        let mut gains = dec!(0);
        let mut losses = dec!(0);

        for i in (0..period).rev() {
            let diff = data[i].close - data[i+1].close;
            if diff > dec!(0) { gains += diff; } else { losses -= diff; }
        }

        if losses == dec!(0) { return Some(dec!(100)); }

        let rs = (gains / Decimal::from(period)) / (losses / Decimal::from(period));
        Some(dec!(100) - (dec!(100) / (dec!(1) + rs)))
    }

    fn rebalance(
        &mut self,
        timestamp: chrono::DateTime<chrono::Utc>,
        time_series_map: &HashMap<Uuid, TimeSeries>,
    ) -> Result<()> {
        Ok(()) // Simplified for integration test
    }
}
