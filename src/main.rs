use axum::Router;
use prometheus::{Counter, Encoder, Gauge, Histogram, Registry, TextEncoder};
use rust_decimal::prelude::*;
use rust_decimal_macros::dec;
use std::net::SocketAddr;
use std::ops::Add;
use std::sync::{Arc, Mutex};
use std::thread;
use tokio_postgres::{Error, NoTls};

struct Metrics {
    connection_errors: Counter,
    seq_scan: Counter,
    seq_tup_read: Counter,
    idx_scan: Counter,
    avg: Counter,
}

#[derive(Debug)]
struct Common_effectiveness {
    relname: String,
    seq_scan: i64,
    seq_tup_read: i64,
    idx_scan: i64,
    avg: i64,
    vacuum_full_count: i64,
    autovacuum_count: i64,
    analyze_count: i64,
    autoanalyze_count: i64,
}

#[derive(Debug)]
struct Index_usage {
    relname: String,
    percent_of_times_index_used: i64,
    rows_in_table: i64,
}

#[derive(Debug)]
struct Hit_miss {
    heap_read: Decimal,
    heap_hit: Decimal,
    ratio: Decimal,
}

impl Metrics {
    fn new() -> Self {
        let registry = Registry::new();
        let seq_scan = Counter::new(
            "seq_scan",
            "Количество последовательных чтений, запущенных по этой таблице",
        )
        .unwrap();
        registry.register(Box::new(seq_scan.clone())).unwrap();
        let seq_tup_read = Counter::new(
            "seq_tup_read",
            "Количество 'живых' строк, прочитанных при последовательных чтениях",
        )
        .unwrap();
        registry.register(Box::new(seq_tup_read.clone())).unwrap();
        let idx_scan = Counter::new(
            "idx_scan",
            "Количество запущенных сканирований по этому индексу",
        )
        .unwrap();
        registry.register(Box::new(idx_scan.clone())).unwrap();
        let avg = Counter::new("avg", "Отношение seq_tup_read к seq_scan").unwrap();
        registry.register(Box::new(avg.clone())).unwrap();
        let connection_errors = Counter::new("connection_errors", "Connection errors").unwrap();
        registry
            .register(Box::new(connection_errors.clone()))
            .unwrap();
        Self {
            seq_scan,
            seq_tup_read,
            idx_scan,
            avg,
            connection_errors,
        }
    }
}

async fn metrics() -> String {
    let (postgres_client, connection) = tokio_postgres::connect(
        "host=common-sentry-21-pg1.alakt.kz.prod.bash.kz user=batman password=moped1 dbname=sentry",
        NoTls,
    )
    .await
    .unwrap();
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });
    let common_effectiveness_statement = postgres_client.prepare("SELECT relname, seq_scan, seq_tup_read, idx_scan, vacuum_count, autovacuum_count, analyze_count, autoanalyze_count, seq_tup_read/seq_scan as avg FROM pg_stat_user_tables WHERE seq_scan > 0 ORDER BY seq_tup_read DESC").await.unwrap();
    let common_effectiveness_rows = postgres_client
        .query(&common_effectiveness_statement, &[])
        .await
        .unwrap();
    let common_effectiveness: Vec<Common_effectiveness> = common_effectiveness_rows
        .iter()
        .map(|row| Common_effectiveness {
            relname: row.get(0),
            seq_scan: row.get(1),
            seq_tup_read: row.get(2),
            idx_scan: row.get(3),
            vacuum_full_count: row.get(4),
            autovacuum_count: row.get(5),
            analyze_count: row.get(6),
            autoanalyze_count: row.get(7),
            avg: row.get(8),
        })
        .collect();
    let hit_miss_statement = postgres_client.prepare("SELECT sum(heap_blks_read) as heap_read, sum(heap_blks_hit) as heap_hit, sum(heap_blks_hit) / (sum(heap_blks_hit) + sum(heap_blks_read)) as ratio FROM pg_statio_user_tables").await.unwrap();
    let hit_miss_rows = postgres_client
        .query(&hit_miss_statement, &[])
        .await
        .unwrap();
    let hit_miss: Vec<Hit_miss> = hit_miss_rows
        .iter()
        .map(|row| Hit_miss {
            heap_read: row.get(0),
            heap_hit: row.get(1),
            ratio: row.get(2),
        })
        .collect();
    let index_usage_statement = postgres_client
        .prepare(
            "SELECT
          relname,
          100 * idx_scan / (seq_scan + idx_scan) percent_of_times_index_used,
          n_live_tup rows_in_table
        FROM
          pg_stat_user_tables
        WHERE
            seq_scan + idx_scan > 0
        ORDER BY
          n_live_tup DESC;",
        )
        .await
        .unwrap();
    let index_usage_rows = postgres_client
        .query(&index_usage_statement, &[])
        .await
        .unwrap();
    let index_usage: Vec<Index_usage> = index_usage_rows
        .iter()
        .map(|row| Index_usage {
            relname: row.get(0),
            percent_of_times_index_used: row.get(1),
            rows_in_table: row.get(2),
        })
        .collect();
    let mut result = String::new();
    for i in common_effectiveness {
        result = result
            + format!(
                "test.postgresql.common_effectiveness.seq_scan.{}: {}\n",
                i.relname, i.seq_scan
            )
            .as_str();
        result = result
            + format!(
                "test.postgresql.common_effectiveness.seq_tup_read.{}: {}\n",
                i.relname, i.seq_tup_read
            )
            .as_str();
        result = result
            + format!(
                "test.postgresql.common_effectiveness.idx_scan.{}: {}\n",
                i.relname, i.idx_scan
            )
            .as_str();
        result = result
            + format!(
                "test.postgresql.common_effectiveness.vacuum_full_count.{}: {}\n",
                i.relname, i.vacuum_full_count
            )
            .as_str();
        result = result
            + format!(
                "test.postgresql.common_effectiveness.autovacuum_count.{}: {}\n",
                i.relname, i.autovacuum_count
            )
            .as_str();
        result = result
            + format!(
                "test.postgresql.common_effectiveness.analyze_count.{}: {}\n",
                i.relname, i.analyze_count
            )
            .as_str();
        result = result
            + format!(
                "test.postgresql.common_effectiveness.autoanalyze_count.{}: {}\n",
                i.relname, i.autoanalyze_count
            )
            .as_str();
        result = result
            + format!(
                "test.postgresql.common_effectiveness.avg.{}: {}\n",
                i.relname, i.avg
            )
            .as_str();
    }
    for i in hit_miss {
        result = result
            + format!(
                "test.postgresql.common_effectiveness.heap_read: {}\n",
                i.heap_read
            )
            .as_str();
        result = result
            + format!(
                "test.postgresql.common_effectiveness.heap_hit: {}\n",
                i.heap_hit
            )
            .as_str();
        result =
            result + format!("test.postgresql.common_effectiveness.ratio: {}\n", i.ratio).as_str();
    }
    for i in index_usage {
        result = result
            + format!(
                "test.postgresql.common_effectiveness.percent_of_times_index_used.{}: {}\n",
                i.relname, i.percent_of_times_index_used
            )
            .as_str();
        result = result
            + format!(
                "test.postgresql.common_effectiveness.rows_in_table.{}: {}\n",
                i.relname, i.rows_in_table
            )
            .as_str();
    }
    result
}

#[tokio::main]
async fn main() {
    let server = SocketAddr::from(([127, 0, 0, 1], 8080));
    let app = Router::new().route("/metrics", axum::routing::get(metrics));
    // Start server.
    axum::Server::bind(&server)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
