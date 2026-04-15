use crate::domain::*;
use anyhow::Result;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::HashMap;
use uuid::Uuid;

pub struct BacktestEngine {
    pub portfolio: Portfolio,
    pub history: Vec<PortfolioSnapshot>,
    pub trades: Vec<Order>,
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
        }
    }

    pub fn run(
        &mut self,
        strategy: &Strategy,
        time_series_map: &HashMap<Uuid, TimeSeries>,
    ) -> Result<()> {
        // Collect all unique timestamps across all assets and sort them
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

        // 2. Evaluate Exit Rules for current positions
        let mut to_sell = Vec::new();
        for asset_id in self.portfolio.positions.keys() {
            if self.should_exit(*asset_id, timestamp, strategy, time_series_map) {
                to_sell.push(*asset_id);
            }
        }

        for asset_id in to_sell {
            self.execute_sell(asset_id, timestamp, time_series_map)?;
        }

        // 3. Evaluate Entry Rules
        for asset_id in time_series_map.keys() {
            if !self.portfolio.positions.contains_key(asset_id) {
                if self.should_enter(*asset_id, timestamp, strategy, time_series_map) {
                    self.execute_buy(*asset_id, timestamp, time_series_map)?;
                }
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

    fn should_enter(
        &self,
        asset_id: Uuid,
        timestamp: chrono::DateTime<chrono::Utc>,
        strategy: &Strategy,
        time_series_map: &HashMap<Uuid, TimeSeries>,
    ) -> bool {
        if strategy.entry_rules.is_empty() {
            return false;
        }
        strategy.entry_rules.iter().all(|rule| {
            self.evaluate_rule(rule, asset_id, timestamp, time_series_map)
        })
    }

    fn should_exit(
        &self,
        asset_id: Uuid,
        timestamp: chrono::DateTime<chrono::Utc>,
        strategy: &Strategy,
        time_series_map: &HashMap<Uuid, TimeSeries>,
    ) -> bool {
        if strategy.exit_rules.is_empty() {
            return false;
        }
        strategy.exit_rules.iter().any(|rule| {
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
                _ => false, // Simplification for MVP
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
            _ => None,
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
            // Spend 10% of cash for simplicity in this MVP
            let amount_to_spend = self.portfolio.cash * dec!(0.1);
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
}
