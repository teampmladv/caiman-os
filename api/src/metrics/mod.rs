//! metrics/mod.rs — Prometheus metrics exporter
//!
//! Serves /metrics on port 9090 for Prometheus to scrape.
//!
//! Gauges exported:
//!   caiman_node_cpu_pct{node}         Node CPU utilization %
//!   caiman_node_mem_used_mib{node}    Node RAM used MiB
//!   caiman_node_load_score{node}      DRS composite load score [0-1]
//!   caiman_vm_cpu_pct{vm,node}        VM CPU utilization %
//!   caiman_vm_mem_mib{vm,node}        VM RAM used MiB
//!   caiman_vm_net_rx_mbps{vm,node}    VM network RX Mbps (XDP)
//!   caiman_vm_net_tx_mbps{vm,node}    VM network TX Mbps (XDP)
//!   caiman_vm_net_drops{vm,node}      VM XDP RX drop counter
//!   caiman_cluster_balance_sigma      DRS balance σ
//!   caiman_xdp_throughput_gbps        Total cluster XDP throughput
//!   caiman_microseg_denies_total{src,dst}  Micro-seg deny events (counter)
//!   caiman_vm_status{vm,status}       VM status as gauge (1=active)

use std::sync::Arc;
use std::net::SocketAddr;
use prometheus::{
    Registry, Gauge, GaugeVec, IntCounterVec, IntGaugeVec,
    Encoder, TextEncoder,
    opts,
};
use axum::{Router, routing::get, extract::State};
use tracing::info;

use crate::state::AppState;

pub struct Metrics {
    pub registry: Registry,

    // Node metrics
    pub node_cpu_pct:      GaugeVec,
    pub node_mem_used:     IntGaugeVec,
    pub node_load_score:   GaugeVec,

    // VM metrics
    pub vm_cpu_pct:        GaugeVec,
    pub vm_mem_mib:        IntGaugeVec,
    pub vm_net_rx_mbps:    GaugeVec,
    pub vm_net_tx_mbps:    GaugeVec,
    pub vm_net_drops:      IntCounterVec,
    pub vm_status:         IntGaugeVec,

    // Cluster metrics
    pub cluster_sigma:     Gauge,
    pub xdp_throughput:    Gauge,
    pub microseg_denies:   IntCounterVec,

    // Uptime / info
    pub vm_uptime_secs:    IntGaugeVec,
}

impl Metrics {
    pub fn new() -> anyhow::Result<Self> {
        let registry = Registry::new();

        macro_rules! gauge_vec {
            ($name:expr, $help:expr, $labels:expr) => {{
                let g = GaugeVec::new(opts!($name, $help), $labels)?;
                registry.register(Box::new(g.clone()))?;
                g
            }};
        }
        macro_rules! int_gauge_vec {
            ($name:expr, $help:expr, $labels:expr) => {{
                let g = IntGaugeVec::new(opts!($name, $help), $labels)?;
                registry.register(Box::new(g.clone()))?;
                g
            }};
        }
        macro_rules! int_counter_vec {
            ($name:expr, $help:expr, $labels:expr) => {{
                let c = IntCounterVec::new(opts!($name, $help), $labels)?;
                registry.register(Box::new(c.clone()))?;
                c
            }};
        }
        macro_rules! gauge {
            ($name:expr, $help:expr) => {{
                let g = Gauge::new($name, $help)?;
                registry.register(Box::new(g.clone()))?;
                g
            }};
        }

        Ok(Self {
            registry,
            node_cpu_pct:    gauge_vec!("caiman_node_cpu_pct",     "Node CPU utilization %",    &["node"]),
            node_mem_used:   int_gauge_vec!("caiman_node_mem_used_mib", "Node RAM used MiB",     &["node"]),
            node_load_score: gauge_vec!("caiman_node_load_score",   "DRS composite load [0-1]", &["node"]),
            vm_cpu_pct:      gauge_vec!("caiman_vm_cpu_pct",        "VM CPU utilization %",     &["vm", "node"]),
            vm_mem_mib:      int_gauge_vec!("caiman_vm_mem_mib",    "VM RAM used MiB",          &["vm", "node"]),
            vm_net_rx_mbps:  gauge_vec!("caiman_vm_net_rx_mbps",    "VM XDP RX Mbps",           &["vm", "node"]),
            vm_net_tx_mbps:  gauge_vec!("caiman_vm_net_tx_mbps",    "VM XDP TX Mbps",           &["vm", "node"]),
            vm_net_drops:    int_counter_vec!("caiman_vm_net_drops","VM XDP drop counter",      &["vm", "node"]),
            vm_status:       int_gauge_vec!("caiman_vm_status",     "VM status (1=running)",    &["vm", "status"]),
            vm_uptime_secs:  int_gauge_vec!("caiman_vm_uptime_secs","VM uptime seconds",        &["vm"]),
            cluster_sigma:   gauge!("caiman_cluster_balance_sigma", "DRS cluster balance σ"),
            xdp_throughput:  gauge!("caiman_xdp_throughput_gbps",   "Total XDP throughput Gbps"),
            microseg_denies: int_counter_vec!("caiman_microseg_denies_total",
                                              "Micro-seg deny events", &["src_id", "dst_id"]),
        })
    }

    pub fn update_from_state(&self, state: &crate::collectors::ClusterState) {
        // Update node metrics
        for node in &state.nodes {
            let n = node.hostname.as_str();
            self.node_cpu_pct    .with_label_values(&[n]).set(node.cpu_usage_pct);
            self.node_mem_used   .with_label_values(&[n]).set(node.mem_used_mib as i64);
            self.node_load_score .with_label_values(&[n]).set(node.load_score);
        }

        // Update VM metrics
        for vm in &state.vms {
            let v = vm.id.as_str();
            let n = vm.node_name.as_str();
            self.vm_cpu_pct    .with_label_values(&[v, n]).set(vm.cpu_usage_pct);
            self.vm_mem_mib    .with_label_values(&[v, n]).set(vm.mem_mib as i64);
            self.vm_net_rx_mbps.with_label_values(&[v, n]).set(vm.net_rx_mbps);
            self.vm_net_tx_mbps.with_label_values(&[v, n]).set(vm.net_tx_mbps);
            self.vm_uptime_secs.with_label_values(&[v]).set(vm.uptime_secs as i64);

            // Status gauge
            for status in &["RUNNING", "MIGRATING", "STOPPED", "BOOTING", "ERROR"] {
                self.vm_status.with_label_values(&[v, status])
                    .set(if vm.status == *status { 1 } else { 0 });
            }
        }

        // Cluster-level
        self.cluster_sigma .set(state.balance_sigma);
        self.xdp_throughput.set(state.xdp_throughput_gbps);
    }
}

// ── HTTP metrics server ────────────────────────────────────────────────────

pub async fn serve_metrics(state: Arc<AppState>, port: u16) {
    let metrics = Arc::new(Metrics::new().expect("creating Prometheus metrics"));

    let app = Router::new()
        .route("/metrics", get(metrics_handler))
        .with_state((state, metrics));

    let addr: SocketAddr = format!("0.0.0.0:{port}").parse().unwrap();
    info!("Prometheus metrics on http://{addr}/metrics");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn metrics_handler(
    axum::extract::State((state, metrics)): axum::extract::State<(Arc<AppState>, Arc<Metrics>)>,
) -> String {
    // Refresh metrics from current state
    let snap = state.cluster.read().await;
    metrics.update_from_state(&snap);
    drop(snap);

    // Encode to Prometheus text format
    let encoder = TextEncoder::new();
    let metric_families = metrics.registry.gather();
    let mut buf = Vec::new();
    encoder.encode(&metric_families, &mut buf).unwrap_or_default();

    // Also include default process metrics
    let default_families = prometheus::gather();
    encoder.encode(&default_families, &mut buf).unwrap_or_default();

    String::from_utf8(buf).unwrap_or_default()
}
