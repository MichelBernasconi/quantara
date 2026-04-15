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

        // 4. Handle Rebalancing
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
            Indicator::RSI(period) => self.calculate_rsi(asset_id, timestamp, *period, time_series_map),
            Indicator::Value(val) => Some(*val),
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
            // Spend 100% of cash for this demonstration
            let amount_to_spend = self.portfolio.cash * dec!(1.0);
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
        
        if data.len() < period + 1 {
            return None;
        }

        let mut gains = dec!(0);
        let mut losses = dec!(0);

        // Calculate gains/losses from previous price to current
        for i in (0..period).rev() {
            let diff = data[i].close - data[i+1].close;
            if diff > dec!(0) {
                gains += diff;
            } else {
                losses -= diff;
            }
        }

        if losses == dec!(0) {
            return Some(dec!(100));
        }

        let rs = (gains / Decimal::from(period)) / (losses / Decimal::from(period));
        Some(dec!(100) - (dec!(100) / (dec!(1) + rs)))
    }

    fn rebalance(
        &mut self,
        timestamp: chrono::DateTime<chrono::Utc>,
        time_series_map: &HashMap<Uuid, TimeSeries>,
    ) -> Result<()> {
        let total_value = self.history.last().map(|s| s.total_value).unwrap_or(dec!(0));
        if total_value == dec!(0) || self.portfolio.positions.is_empty() {
            return Ok(());
        }

        let target_value_per_asset = total_value / Decimal::from(self.portfolio.positions.len());

        let asset_ids: Vec<Uuid> = self.portfolio.positions.keys().cloned().collect();
        for asset_id in asset_ids {
            let current_pos = self.portfolio.positions.get(&asset_id).unwrap();
            let price = self.get_price_at(asset_id, timestamp, time_series_map).unwrap_or(dec!(0));
            if price == dec!(0) { continue; }

            let current_value = current_pos.quantity * price;
            if (current_value - target_value_per_asset).abs() > (target_value_per_asset * dec!(0.05)) {
                if current_value > target_value_per_asset {
                    let to_sell = (current_value - target_value_per_asset) / price;
                    self.execute_partial_sell(asset_id, to_sell, price, timestamp)?;
                } else {
                    let to_buy = (target_value_per_asset - current_value) / price;
                    if self.portfolio.cash >= to_buy * price {
                        self.execute_partial_buy(asset_id, to_buy, price, timestamp)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn execute_partial_buy(&mut self, asset_id: Uuid, quantity: Decimal, price: Decimal, timestamp: chrono::DateTime<chrono::Utc>) -> Result<()> {
        self.portfolio.cash -= quantity * price;
        let entry = self.portfolio.positions.entry(asset_id).or_insert(Position {
            asset_id,
            quantity: dec!(0),
            average_price: dec!(0),
        });
        entry.average_price = (entry.average_price * entry.quantity + price * quantity) / (entry.quantity + quantity);
        entry.quantity += quantity;
        self.trades.push(Order { id: Uuid::new_v4(), asset_id, quantity, price, timestamp, side: OrderSide::Buy });
        Ok(())
    }

    fn execute_partial_sell(&mut self, asset_id: Uuid, quantity: Decimal, price: Decimal, timestamp: chrono::DateTime<chrono::Utc>) -> Result<()> {
        let entry = self.portfolio.positions.get_mut(&asset_id).unwrap();
        entry.quantity -= quantity;
        self.portfolio.cash += quantity * price;
        self.trades.push(Order { id: Uuid::new_v4(), asset_id, quantity, price, timestamp, side: OrderSide::Sell });
        let is_empty = entry.quantity <= dec!(0);
        if is_empty { self.portfolio.positions.remove(&asset_id); }
        Ok(())
    }
}
