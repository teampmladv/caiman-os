//! caiman-gpu v0.8.0 — GPU daemon
//! Manages NVIDIA GPU passthrough, MIG slices, and vGPU for Caimán VMs.
//!
//! Three modes:
//!   passthrough  Full GPU via VFIO-PCI (best perf, exclusive)
//!   mig          Hardware MIG slice (A100/H100, isolated)
//!   vgpu         Software vGPU via mdev (best density)
//!
//! Runs as a daemon on each node with a GPU.
//! caiman-api calls it via REST on port 8769.

use axum::{Router, routing::get, Json, extract::Path, http::StatusCode, response::IntoResponse};
use serde_json::{json, Value};
use std::net::SocketAddr;
use tracing::info;

pub mod mig;
pub mod passthrough;
pub mod vgpu;

use mig::MigProfile;
use vgpu::VgpuProfile;

// ── GPU device types ──────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GpuDevice {
    pub pci_address:  String,
    pub gpu_uuid:     String,
    pub model:        String,
    pub vram_mib:     u64,
    pub driver_ver:   String,
    pub cuda_ver:     String,
    pub mig_capable:  bool,
    pub vgpu_capable: bool,
    pub iommu_group:  u32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum GpuAllocation {
    Passthrough { gpu: GpuDevice, vfio_dev: String },
    MigSlice    { gpu: GpuDevice, profile: MigProfile, instance_id: u32, vfio_dev: String },
    VGpu        { gpu: GpuDevice, profile: VgpuProfile, vgpu_id: u32, mdev_path: String },
}

// ── HTTP handlers ─────────────────────────────────────────────────────────

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok", "service": "caiman-gpu", "version": env!("CARGO_PKG_VERSION") }))
}

async fn list_gpus() -> impl IntoResponse {
    match discover_gpus().await {
        Ok(gpus) => (StatusCode::OK, Json(json!(gpus))).into_response(),
        Err(e)   => (StatusCode::SERVICE_UNAVAILABLE,
                     Json(json!({ "error": e.to_string(), "gpus": [] }))).into_response(),
    }
}

async fn list_mig_profiles(Path(pci): Path<String>) -> impl IntoResponse {
    let gpu = GpuDevice {
        pci_address: pci, gpu_uuid: String::new(), model: String::new(),
        vram_mib: 0, driver_ver: String::new(), cuda_ver: String::new(),
        mig_capable: true, vgpu_capable: false, iommu_group: 0,
    };
    match mig::list_profiles(&gpu).await {
        Ok(profiles) => (StatusCode::OK, Json(json!(profiles))).into_response(),
        Err(e)       => (StatusCode::INTERNAL_SERVER_ERROR,
                         Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn list_vgpu_profiles(Path(pci): Path<String>) -> impl IntoResponse {
    let gpu = GpuDevice {
        pci_address: pci, gpu_uuid: String::new(), model: String::new(),
        vram_mib: 0, driver_ver: String::new(), cuda_ver: String::new(),
        mig_capable: false, vgpu_capable: true, iommu_group: 0,
    };
    match vgpu::list_profiles(&gpu).await {
        Ok(profiles) => (StatusCode::OK, Json(json!(profiles))).into_response(),
        Err(e)       => (StatusCode::INTERNAL_SERVER_ERROR,
                         Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn node_gpu_summary() -> Json<Value> {
    let gpus = discover_gpus().await.unwrap_or_default();
    let total     = gpus.len();
    let mig_ready = gpus.iter().filter(|g| g.mig_capable).count();
    let vgpu_ready= gpus.iter().filter(|g| g.vgpu_capable).count();
    let total_vram: u64 = gpus.iter().map(|g| g.vram_mib).sum();

    Json(json!({
        "totalGpus":    total,
        "migCapable":   mig_ready,
        "vgpuCapable":  vgpu_ready,
        "totalVramMib": total_vram,
        "gpus":         gpus,
    }))
}

// ── GPU discovery ─────────────────────────────────────────────────────────

async fn discover_gpus() -> anyhow::Result<Vec<GpuDevice>> {
    let out = tokio::process::Command::new("nvidia-smi")
        .args([
            "--query-gpu=pci.bus_id,gpu_uuid,name,memory.total,driver_version,compute_cap",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .await
        .map_err(|_| anyhow::anyhow!("nvidia-smi not found — no NVIDIA GPU or driver"))?;

    let stdout = String::from_utf8(out.stdout)?;
    let mut gpus = Vec::new();

    for line in stdout.lines().filter(|l| !l.is_empty()) {
        let p: Vec<&str> = line.split(", ").collect();
        if p.len() < 5 { continue; }

        let pci = p[0].trim().to_string();
        let iommu = std::fs::read_link(
            format!("/sys/bus/pci/devices/{pci}/iommu_group")
        ).ok()
            .and_then(|l| l.file_name().and_then(|n| n.to_str().and_then(|s| s.parse().ok())))
            .unwrap_or(0);

        let mig_capable = tokio::process::Command::new("nvidia-smi")
            .args(["-i", &pci, "--query-gpu=mig.mode.current", "--format=csv,noheader"])
            .output().await
            .map(|o| !String::from_utf8_lossy(&o.stdout).contains("[N/A]"))
            .unwrap_or(false);

        let vgpu_capable = std::path::Path::new(
            &format!("/sys/bus/pci/devices/{pci}/mdev_supported_types")
        ).exists();

        gpus.push(GpuDevice {
            pci_address:  pci,
            gpu_uuid:     p[1].trim().to_string(),
            model:        p[2].trim().to_string(),
            vram_mib:     p[3].trim().parse().unwrap_or(0),
            driver_ver:   p[4].trim().to_string(),
            cuda_ver:     p.get(5).unwrap_or(&"").trim().to_string(),
            mig_capable,
            vgpu_capable,
            iommu_group:  iommu,
        });
    }

    Ok(gpus)
}

// ── Main ──────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let app = Router::new()
        .route("/health",                              get(health))
        .route("/api/gpu",                             get(list_gpus))
        .route("/api/gpu/summary",                     get(node_gpu_summary))
        .route("/api/gpu/:pci/mig/profiles",           get(list_mig_profiles))
        .route("/api/gpu/:pci/vgpu/profiles",          get(list_vgpu_profiles));

    let addr: SocketAddr = "0.0.0.0:8769".parse().unwrap();
    info!("caiman-gpu v{} listening on {addr}", env!("CARGO_PKG_VERSION"));
    info!("GPU modes: passthrough | mig | vgpu");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
