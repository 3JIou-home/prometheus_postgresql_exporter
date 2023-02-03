# prometheus_postgresql_exporter
Usage:

git clone      
cargo build --release   
./target/release/prometheus-postgresql-exporter my-postgresql-ho my-database my-user my-password   
curl localhost:8080/metrics   