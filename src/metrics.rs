use crate::engine::PortfolioSnapshot;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct BacktestMetrics {
    pub total_return: Decimal,
    pub annual_return: Decimal,
    pub max_drawdown: Decimal,
    pub volatility: Decimal,
    pub final_value: Decimal,
}

pub fn calculate_metrics(history: &[PortfolioSnapshot]) -> BacktestMetrics {
    if history.is_empty() {
        return BacktestMetrics {
            total_return: dec!(0),
            annual_return: dec!(0),
            max_drawdown: dec!(0),
            volatility: dec!(0),
            final_value: dec!(0),
        };
    }

    let initial_value = history[0].total_value;
    let final_value = history.last().unwrap().total_value;

    let total_return = if initial_value > dec!(0) {
        (final_value - initial_value) / initial_value
    } else {
        dec!(0)
    };

    // Max Drawdown calculation
    let mut max_drawdown = dec!(0);
    let mut peak = initial_value;

    for snap in history {
        if snap.total_value > peak {
            peak = snap.total_value;
        }
        let dd = (peak - snap.total_value) / peak;
        if dd > max_drawdown {
            max_drawdown = dd;
        }
    }

    // Placeholder for volatility and annual return
    // (Needs more complex math/time difference check)

    BacktestMetrics {
        total_return,
        annual_return: dec!(0), // WIP
        max_drawdown,
        volatility: dec!(0), // WIP
        final_value,
    }
}
