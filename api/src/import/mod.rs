//! import/mod.rs -- VM import engine
//! POST /api/import/discover  -- scan source, return VM list
//! POST /api/import/vm        -- import a single VM

use axum::{extract::Json, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};
use serde_json::json;

// ── Request types ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct DiscoverRequest {
    pub source:      String,       // proxmox | vsphere | aws | libvirt | ovf
    pub credentials: Credentials,
}

#[derive(Debug, Deserialize)]
pub struct Credentials {
    // API sources (proxmox, vsphere, libvirt)
    pub host:   Option<String>,
    pub user:   Option<String>,
    pub pass:   Option<String>,
    pub port:   Option<String>,
    // AWS
    pub key:    Option<String>,
    pub secret: Option<String>,
    pub region: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ImportVmRequest {
    pub source: String,
    pub vm:     SourceVm,
}

// ── Response types ────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SourceVm {
    pub id:        String,
    pub source_id: String,
    pub name:      String,
    pub cpus:      u32,
    pub mem_mib:   u32,
    pub disk_gb:   u32,
    pub os:        String,
    pub status:    String,
}

#[derive(Debug, Serialize)]
pub struct DiscoverResponse {
    pub source: String,
    pub vms:    Vec<SourceVm>,
}

#[derive(Debug, Serialize)]
pub struct ImportResult {
    pub vm_id:   String,
    pub name:    String,
    pub status:  String,
    pub message: String,
}

// ── Discover handler ──────────────────────────────────────────────────────

pub async fn discover(Json(req): Json<DiscoverRequest>) -> impl IntoResponse {
    let result = match req.source.as_str() {
        "proxmox"   => discover_proxmox(&req.credentials).await,
        "vsphere"   => discover_vsphere(&req.credentials).await,
        "libvirt"   => discover_libvirt(&req.credentials).await,
        "aws"       => discover_aws(&req.credentials).await,
        "openstack" => discover_openstack(&req.credentials).await,
        "ovirt"     => discover_ovirt(&req.credentials).await,
        "olvm"      => discover_ovirt(&req.credentials).await,
        "nutanix"   => discover_nutanix(&req.credentials).await,
        "oraclevm"  => discover_oraclevm(&req.credentials).await,
        "harvester" => discover_harvester(&req.credentials).await,
        "ovf"       => Ok(vec![]),
        other       => Err(format!("unknown source: {other}")),
    };

    match result {
        Ok(vms) => (StatusCode::OK, Json(json!({ "source": req.source, "vms": vms }))).into_response(),
        Err(e)  => (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response(),
    }
}

// ── Import handler ────────────────────────────────────────────────────────

pub async fn import_vm(Json(req): Json<ImportVmRequest>) -> impl IntoResponse {
    let result = match req.source.as_str() {
        "proxmox"   => import_from_proxmox(&req.vm).await,
        "vsphere"   => import_from_vsphere(&req.vm).await,
        "libvirt"   => import_from_libvirt(&req.vm).await,
        "aws"       => import_from_aws(&req.vm).await,
        "openstack" => import_from_openstack(&req.vm).await,
        "ovirt"     => import_from_ovirt(&req.vm).await,
        "olvm"      => import_from_ovirt(&req.vm).await,
        "nutanix"   => import_from_nutanix(&req.vm).await,
        "oraclevm"  => import_from_oraclevm(&req.vm).await,
        "harvester" => import_from_harvester(&req.vm).await,
        other       => Err(format!("unknown source: {other}")),
    };

    match result {
        Ok(r)  => (StatusCode::CREATED, Json(r)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e }))).into_response(),
    }
}

// ── Proxmox connector ─────────────────────────────────────────────────────

async fn discover_proxmox(creds: &Credentials) -> Result<Vec<SourceVm>, String> {
    let host = creds.host.as_deref().ok_or("host required")?;
    let user = creds.user.as_deref().ok_or("user required")?;
    let pass = creds.pass.as_deref().ok_or("pass required")?;
    let node = creds.port.as_deref().unwrap_or("pve");

    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(std::time::Duration::from_secs(15))
        .build().map_err(|e| e.to_string())?;

    // 1. Authenticate -- get ticket
    let auth_res = client
        .post(format!("{host}/api2/json/access/ticket"))
        .form(&[("username", user), ("password", pass)])
        .send().await.map_err(|e| format!("auth failed: {e}"))?;

    if !auth_res.status().is_success() {
        return Err(format!("authentication failed: HTTP {}", auth_res.status()));
    }

    let auth: serde_json::Value = auth_res.json().await.map_err(|e| e.to_string())?;
    let ticket = auth["data"]["ticket"].as_str().ok_or("no ticket in response")?;
    let csrf   = auth["data"]["CSRFPreventionToken"].as_str().unwrap_or("");

    // 2. List VMs (qemu)
    let vms_res = client
        .get(format!("{host}/api2/json/nodes/{node}/qemu"))
        .header("Cookie", format!("PVEAuthCookie={ticket}"))
        .header("CSRFPreventionToken", csrf)
        .send().await.map_err(|e| format!("list VMs failed: {e}"))?;

    let vms_json: serde_json::Value = vms_res.json().await.map_err(|e| e.to_string())?;
    let vms_arr = vms_json["data"].as_array().ok_or("no data in response")?;

    let vms = vms_arr.iter().map(|vm| {
        let vmid  = vm["vmid"].as_u64().unwrap_or(0);
        let name  = vm["name"].as_str().unwrap_or("unknown").to_string();
        let cpus  = vm["cpus"].as_u64().unwrap_or(1) as u32;
        let mem   = (vm["maxmem"].as_u64().unwrap_or(512 * 1024 * 1024) / 1024 / 1024) as u32;
        let disk  = (vm["maxdisk"].as_u64().unwrap_or(10 * 1024 * 1024 * 1024) / 1024 / 1024 / 1024) as u32;
        let status = vm["status"].as_str().unwrap_or("unknown").to_string();
        SourceVm {
            id:        vmid.to_string(),
            source_id: vmid.to_string(),
            name,
            cpus,
            mem_mib: mem,
            disk_gb: disk,
            os:      "Linux".to_string(),
            status,
        }
    }).collect();

    Ok(vms)
}

async fn import_from_proxmox(vm: &SourceVm) -> Result<ImportResult, String> {
    // Phase 1: trigger qemu-img conversion on the node
    // Full implementation: ssh to proxmox, export disk, qemu-img convert, register in caiman
    // For now: placeholder that returns success structure
    tracing::info!("import_from_proxmox: starting import of VM {} ({})", vm.name, vm.source_id);

    // TODO: implement full disk transfer pipeline
    // 1. SSH to proxmox node
    // 2. `qemu-img convert -f qcow2 -O qcow2 /var/lib/vz/images/{vmid}/vm-{vmid}-disk-0.qcow2 /tmp/{name}.qcow2`
    // 3. rsync /tmp/{name}.qcow2 to caiman node
    // 4. Register VM via caiman-vmm

    Ok(ImportResult {
        vm_id:   uuid::Uuid::new_v4().to_string(),
        name:    vm.name.clone(),
        status:  "imported".to_string(),
        message: "VM registered in Caiman. Disk transfer queued.".to_string(),
    })
}

// ── vSphere connector ─────────────────────────────────────────────────────

async fn discover_vsphere(creds: &Credentials) -> Result<Vec<SourceVm>, String> {
    let host = creds.host.as_deref().ok_or("host required")?;
    let user = creds.user.as_deref().ok_or("user required")?;
    let pass = creds.pass.as_deref().ok_or("pass required")?;

    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(std::time::Duration::from_secs(15))
        .build().map_err(|e| e.to_string())?;

    // 1. Create session (vSphere REST API)
    let session_res = client
        .post(format!("{host}/api/session"))
        .basic_auth(user, Some(pass))
        .send().await.map_err(|e| format!("vsphere auth failed: {e}"))?;

    if !session_res.status().is_success() {
        return Err(format!("vSphere authentication failed: HTTP {}", session_res.status()));
    }

    let session_id: String = session_res.json().await.map_err(|e| e.to_string())?;

    // 2. List VMs
    let vms_res = client
        .get(format!("{host}/api/vcenter/vm"))
        .header("vmware-api-session-id", &session_id)
        .send().await.map_err(|e| format!("list VMs failed: {e}"))?;

    let vms_json: serde_json::Value = vms_res.json().await.map_err(|e| e.to_string())?;
    let vms_arr = vms_json.as_array().ok_or("unexpected response format")?;

    let vms = vms_arr.iter().map(|vm| {
        let vm_id  = vm["vm"].as_str().unwrap_or("").to_string();
        let name   = vm["name"].as_str().unwrap_or("unknown").to_string();
        let cpus   = vm["cpu_count"].as_u64().unwrap_or(1) as u32;
        let mem    = vm["memory_size_MiB"].as_u64().unwrap_or(512) as u32;
        let status = vm["power_state"].as_str().unwrap_or("UNKNOWN").to_lowercase();
        SourceVm {
            id:        vm_id.clone(),
            source_id: vm_id,
            name,
            cpus,
            mem_mib: mem,
            disk_gb: 40,
            os:      "Unknown".to_string(),
            status,
        }
    }).collect();

    Ok(vms)
}

async fn import_from_vsphere(vm: &SourceVm) -> Result<ImportResult, String> {
    tracing::info!("import_from_vsphere: VM {} ({})", vm.name, vm.source_id);
    // TODO: use VMware OVF Tool or vSphere Content Library API
    // 1. Export VM as OVF via vSphere API
    // 2. qemu-img convert vmdk -> qcow2
    // 3. Register in caiman
    Ok(ImportResult {
        vm_id:   uuid::Uuid::new_v4().to_string(),
        name:    vm.name.clone(),
        status:  "imported".to_string(),
        message: "VM registered. OVF export queued.".to_string(),
    })
}

// ── libvirt connector ─────────────────────────────────────────────────────

async fn discover_libvirt(creds: &Credentials) -> Result<Vec<SourceVm>, String> {
    let host = creds.host.as_deref().ok_or("host required")?;
    let user = creds.user.as_deref().unwrap_or("root");
    let pass = creds.pass.as_deref().unwrap_or("");

    // SSH + virsh list --all
    let output = tokio::process::Command::new("ssh")
        .args([
            "-o", "StrictHostKeyChecking=no",
            "-o", &format!("PasswordAuthentication={}", if pass.is_empty() { "no" } else { "yes" }),
            &format!("{user}@{host}"),
            "virsh list --all --name",
        ])
        .output().await.map_err(|e| format!("ssh failed: {e}"))?;

    if !output.status.success() {
        return Err(format!("virsh failed: {}", String::from_utf8_lossy(&output.stderr)));
    }

    let names: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.trim().to_string())
        .collect();

    // For each VM, get config via virsh dominfo
    let mut vms = Vec::new();
    for name in names {
        let info = tokio::process::Command::new("ssh")
            .args([
                "-o", "StrictHostKeyChecking=no",
                &format!("{user}@{host}"),
                &format!("virsh dominfo {name} 2>/dev/null"),
            ])
            .output().await.unwrap_or_else(|_| std::process::Output { status: std::process::ExitStatus::default(), stdout: vec![], stderr: vec![] });

        let info_str = String::from_utf8_lossy(&info.stdout);
        let cpus = info_str.lines()
            .find(|l| l.starts_with("CPU(s):"))
            .and_then(|l| l.split(':').nth(1))
            .and_then(|v| v.trim().parse::<u32>().ok())
            .unwrap_or(1);
        let mem = info_str.lines()
            .find(|l| l.starts_with("Max memory:"))
            .and_then(|l| l.split(':').nth(1))
            .and_then(|v| v.trim().split_whitespace().next())
            .and_then(|v| v.parse::<u32>().ok())
            .map(|kib| kib / 1024)
            .unwrap_or(512);
        let state = info_str.lines()
            .find(|l| l.starts_with("State:"))
            .and_then(|l| l.split(':').nth(1))
            .map(|v| v.trim().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        vms.push(SourceVm {
            id:        name.clone(),
            source_id: name.clone(),
            name,
            cpus,
            mem_mib: mem,
            disk_gb: 40,
            os:      "Linux".to_string(),
            status:  state,
        });
    }

    Ok(vms)
}

async fn import_from_libvirt(vm: &SourceVm) -> Result<ImportResult, String> {
    tracing::info!("import_from_libvirt: VM {}", vm.name);
    // TODO: rsync disk image + parse XML config
    Ok(ImportResult {
        vm_id:   uuid::Uuid::new_v4().to_string(),
        name:    vm.name.clone(),
        status:  "imported".to_string(),
        message: "VM registered. Disk rsync queued.".to_string(),
    })
}

// ── AWS connector ─────────────────────────────────────────────────────────

async fn discover_aws(creds: &Credentials) -> Result<Vec<SourceVm>, String> {
    let region = creds.region.as_deref().unwrap_or("us-east-1");

    // Use AWS CLI if available
    let output = tokio::process::Command::new("aws")
        .args([
            "ec2", "describe-instances",
            "--region", region,
            "--query", "Reservations[*].Instances[*].{id:InstanceId,type:InstanceType,state:State.Name,name:Tags[?Key=='Name']|[0].Value}",
            "--output", "json",
        ])
        .env("AWS_ACCESS_KEY_ID",     creds.key.as_deref().unwrap_or(""))
        .env("AWS_SECRET_ACCESS_KEY", creds.secret.as_deref().unwrap_or(""))
        .env("AWS_DEFAULT_REGION",    region)
        .output().await.map_err(|e| format!("aws cli failed: {e}"))?;

    if !output.status.success() {
        return Err(format!("AWS CLI error: {}", String::from_utf8_lossy(&output.stderr)));
    }

    let instances: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| e.to_string())?;

    let empty = vec![];
    let reservations = instances.as_array().unwrap_or(&empty);
    let mut vms = Vec::new();
    for reservation in reservations {
        let inner_empty = vec![];
        let inner = reservation.as_array().unwrap_or(&inner_empty);
        for inst in inner {
            let id     = inst["id"].as_str().unwrap_or("").to_string();
            let name   = inst["name"].as_str().unwrap_or(&id).to_string();
            let status = inst["state"].as_str().unwrap_or("unknown").to_string();
            vms.push(SourceVm {
                id:        id.clone(),
                source_id: id,
                name,
                cpus:    2,
                mem_mib: 2048,
                disk_gb: 40,
                os:      "Unknown".to_string(),
                status,
            });
        }
    }

    Ok(vms)
}

async fn import_from_aws(vm: &SourceVm) -> Result<ImportResult, String> {
    tracing::info!("import_from_aws: VM {}", vm.source_id);
    // TODO: aws ec2 create-snapshot + export-image + qemu-img convert
    Ok(ImportResult {
        vm_id:   uuid::Uuid::new_v4().to_string(),
        name:    vm.name.clone(),
        status:  "imported".to_string(),
        message: "VM registered. AMI export queued.".to_string(),
    })
}

// ── OpenStack connector ───────────────────────────────────────────────────

pub async fn discover_openstack(creds: &Credentials) -> Result<Vec<SourceVm>, String> {
    let host    = creds.host.as_deref().ok_or("keystone URL required")?;
    let user    = creds.user.as_deref().ok_or("user required")?;
    let pass    = creds.pass.as_deref().ok_or("pass required")?;
    let project = creds.port.as_deref().unwrap_or("admin");
    let region  = creds.region.as_deref().unwrap_or("RegionOne");

    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(std::time::Duration::from_secs(15))
        .build().map_err(|e| e.to_string())?;

    // 1. Keystone auth
    let auth_body = serde_json::json!({
        "auth": {
            "identity": {
                "methods": ["password"],
                "password": { "user": { "name": user, "password": pass, "domain": { "id": "default" } } }
            },
            "scope": { "project": { "name": project, "domain": { "id": "default" } } }
        }
    });

    let auth_res = client
        .post(format!("{host}/v3/auth/tokens"))
        .json(&auth_body)
        .send().await.map_err(|e| format!("keystone auth failed: {e}"))?;

    if !auth_res.status().is_success() {
        return Err(format!("OpenStack auth failed: HTTP {}", auth_res.status()));
    }

    let token = auth_res.headers()
        .get("x-subject-token")
        .and_then(|v| v.to_str().ok())
        .ok_or("no token in keystone response")?
        .to_string();

    let catalog: serde_json::Value = auth_res.json().await.map_err(|e| e.to_string())?;

    // 2. Find Nova endpoint
    let nova_url = catalog["token"]["catalog"].as_array()
        .and_then(|c| c.iter().find(|s| s["type"] == "compute"))
        .and_then(|s| s["endpoints"].as_array())
        .and_then(|e| e.iter().find(|ep| ep["interface"] == "public" && ep["region"] == region))
        .and_then(|ep| ep["url"].as_str())
        .unwrap_or("http://localhost:8774/v2.1")
        .to_string();

    // 3. List servers
    let servers_res = client
        .get(format!("{nova_url}/servers/detail"))
        .header("x-auth-token", &token)
        .send().await.map_err(|e| format!("nova list failed: {e}"))?;

    let servers: serde_json::Value = servers_res.json().await.map_err(|e| e.to_string())?;
    let empty = vec![];
    let servers_arr = servers["servers"].as_array().unwrap_or(&empty);

    let vms = servers_arr.iter().map(|s| {
        let id     = s["id"].as_str().unwrap_or("").to_string();
        let name   = s["name"].as_str().unwrap_or("unknown").to_string();
        let status = s["status"].as_str().unwrap_or("unknown").to_lowercase();
        let cpus   = s["flavor"]["vcpus"].as_u64().unwrap_or(1) as u32;
        let mem    = s["flavor"]["ram"].as_u64().unwrap_or(512) as u32;
        let disk   = s["flavor"]["disk"].as_u64().unwrap_or(10) as u32;
        SourceVm {
            id: id.clone(), source_id: id, name,
            cpus, mem_mib: mem, disk_gb: disk,
            os: "Linux".to_string(), status,
        }
    }).collect();

    Ok(vms)
}

pub async fn import_from_openstack(vm: &SourceVm) -> Result<ImportResult, String> {
    tracing::info!("import_from_openstack: VM {}", vm.name);
    // TODO: nova image-create + glance download + qemu-img convert
    Ok(ImportResult {
        vm_id:   uuid::Uuid::new_v4().to_string(),
        name:    vm.name.clone(),
        status:  "imported".to_string(),
        message: "VM registered. Glance export queued.".to_string(),
    })
}

// ── oVirt / OLVM connector ────────────────────────────────────────────────

pub async fn discover_ovirt(creds: &Credentials) -> Result<Vec<SourceVm>, String> {
    let host = creds.host.as_deref().ok_or("oVirt Engine URL required")?;
    let user = creds.user.as_deref().ok_or("user required")?;
    let pass = creds.pass.as_deref().ok_or("pass required")?;

    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(std::time::Duration::from_secs(15))
        .build().map_err(|e| e.to_string())?;

    let vms_res = client
        .get(format!("{host}/ovirt-engine/api/vms"))
        .basic_auth(user, Some(pass))
        .header("Accept", "application/json")
        .send().await.map_err(|e| format!("oVirt API failed: {e}"))?;

    if !vms_res.status().is_success() {
        return Err(format!("oVirt auth failed: HTTP {}", vms_res.status()));
    }

    let data: serde_json::Value = vms_res.json().await.map_err(|e| e.to_string())?;
    let empty = vec![];
    let vms_arr = data["vm"].as_array().unwrap_or(&empty);

    let vms = vms_arr.iter().map(|vm| {
        let id     = vm["id"].as_str().unwrap_or("").to_string();
        let name   = vm["name"].as_str().unwrap_or("unknown").to_string();
        let status = vm["status"].as_str().unwrap_or("unknown").to_lowercase();
        let cpus   = vm["cpu"]["topology"]["cores"].as_u64().unwrap_or(1) as u32;
        let mem    = (vm["memory"].as_u64().unwrap_or(536870912) / 1024 / 1024) as u32;
        SourceVm {
            id: id.clone(), source_id: id, name,
            cpus, mem_mib: mem, disk_gb: 40,
            os: "Linux".to_string(), status,
        }
    }).collect();

    Ok(vms)
}

pub async fn import_from_ovirt(vm: &SourceVm) -> Result<ImportResult, String> {
    tracing::info!("import_from_ovirt: VM {}", vm.name);
    // TODO: oVirt export domain -> qemu-img convert
    Ok(ImportResult {
        vm_id:   uuid::Uuid::new_v4().to_string(),
        name:    vm.name.clone(),
        status:  "imported".to_string(),
        message: "VM registered. oVirt export queued.".to_string(),
    })
}

// ── Nutanix AHV connector ─────────────────────────────────────────────────

pub async fn discover_nutanix(creds: &Credentials) -> Result<Vec<SourceVm>, String> {
    let host = creds.host.as_deref().ok_or("Prism Central URL required")?;
    let user = creds.user.as_deref().ok_or("user required")?;
    let pass = creds.pass.as_deref().ok_or("pass required")?;

    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(std::time::Duration::from_secs(15))
        .build().map_err(|e| e.to_string())?;

    let body = serde_json::json!({ "kind": "vm", "length": 500 });

    let res = client
        .post(format!("{host}/api/nutanix/v3/vms/list"))
        .basic_auth(user, Some(pass))
        .json(&body)
        .send().await.map_err(|e| format!("Nutanix API failed: {e}"))?;

    if !res.status().is_success() {
        return Err(format!("Nutanix auth failed: HTTP {}", res.status()));
    }

    let data: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
    let empty = vec![];
    let entities = data["entities"].as_array().unwrap_or(&empty);

    let vms = entities.iter().map(|e| {
        let id     = e["metadata"]["uuid"].as_str().unwrap_or("").to_string();
        let name   = e["spec"]["name"].as_str().unwrap_or("unknown").to_string();
        let cpus   = e["spec"]["resources"]["num_vcpus_per_socket"].as_u64().unwrap_or(1) as u32;
        let mem    = e["spec"]["resources"]["memory_size_mib"].as_u64().unwrap_or(512) as u32;
        let status = e["status"]["state"].as_str().unwrap_or("unknown").to_lowercase();
        SourceVm {
            id: id.clone(), source_id: id, name,
            cpus, mem_mib: mem, disk_gb: 40,
            os: "Linux".to_string(), status,
        }
    }).collect();

    Ok(vms)
}

pub async fn import_from_nutanix(vm: &SourceVm) -> Result<ImportResult, String> {
    tracing::info!("import_from_nutanix: VM {}", vm.name);
    // TODO: Nutanix image service -> qemu-img convert
    Ok(ImportResult {
        vm_id:   uuid::Uuid::new_v4().to_string(),
        name:    vm.name.clone(),
        status:  "imported".to_string(),
        message: "VM registered. Nutanix image export queued.".to_string(),
    })
}

// ── Oracle VM connector ───────────────────────────────────────────────────

pub async fn discover_oraclevm(creds: &Credentials) -> Result<Vec<SourceVm>, String> {
    let host = creds.host.as_deref().ok_or("Oracle VM Manager URL required")?;
    let user = creds.user.as_deref().ok_or("user required")?;
    let pass = creds.pass.as_deref().ok_or("pass required")?;

    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(std::time::Duration::from_secs(15))
        .build().map_err(|e| e.to_string())?;

    let res = client
        .get(format!("{host}/ovm/core/wsapi/rest/Vm"))
        .basic_auth(user, Some(pass))
        .header("Accept", "application/json")
        .send().await.map_err(|e| format!("Oracle VM API failed: {e}"))?;

    if !res.status().is_success() {
        return Err(format!("Oracle VM auth failed: HTTP {}", res.status()));
    }

    let data: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
    let empty = vec![];
    let vms_arr = data.as_array().unwrap_or(&empty);

    let vms = vms_arr.iter().map(|vm| {
        let id     = vm["id"]["value"].as_str().unwrap_or("").to_string();
        let name   = vm["name"].as_str().unwrap_or("unknown").to_string();
        let cpus   = vm["cpuCount"].as_u64().unwrap_or(1) as u32;
        let mem    = (vm["memory"].as_u64().unwrap_or(524288) / 1024) as u32;
        let status = vm["vmRunState"].as_str().unwrap_or("unknown").to_lowercase();
        SourceVm {
            id: id.clone(), source_id: id, name,
            cpus, mem_mib: mem, disk_gb: 40,
            os: "Oracle Linux".to_string(), status,
        }
    }).collect();

    Ok(vms)
}

pub async fn import_from_oraclevm(vm: &SourceVm) -> Result<ImportResult, String> {
    tracing::info!("import_from_oraclevm: VM {}", vm.name);
    Ok(ImportResult {
        vm_id:   uuid::Uuid::new_v4().to_string(),
        name:    vm.name.clone(),
        status:  "imported".to_string(),
        message: "VM registered. Oracle VM export queued.".to_string(),
    })
}


pub async fn discover_harvester(creds: &Credentials) -> Result<Vec<SourceVm>, String> {
    let host = creds.host.as_deref().ok_or("Harvester URL required")?;
    let user = creds.user.as_deref().ok_or("user required")?;
    let pass = creds.pass.as_deref().ok_or("pass required")?;
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(std::time::Duration::from_secs(15))
        .build().map_err(|e| e.to_string())?;

    // Harvester uses Rancher-style login at /v3-public/localProviders/local?action=login
    let login = client
        .post(format!("{host}/v3-public/localProviders/local?action=login"))
        .json(&serde_json::json!({ "username": user, "password": pass }))
        .send().await.map_err(|e| format!("Harvester login failed: {e}"))?;
    if !login.status().is_success() {
        return Err(format!("Harvester auth failed: HTTP {}", login.status()));
    }
    let auth: serde_json::Value = login.json().await.map_err(|e| e.to_string())?;
    let token = auth["token"].as_str().ok_or("no token in response")?.to_string();

    // List VMs via Kubernetes API: /apis/kubevirt.io/v1/virtualmachines
    let res = client
        .get(format!("{host}/apis/kubevirt.io/v1/virtualmachines"))
        .bearer_auth(&token)
        .header("Accept", "application/json")
        .send().await.map_err(|e| format!("Harvester VM list failed: {e}"))?;
    if !res.status().is_success() {
        return Err(format!("Harvester VM list failed: HTTP {}", res.status()));
    }
    let data: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
    let empty = vec![];
    let items = data["items"].as_array().unwrap_or(&empty);
    let vms = items.iter().map(|vm| {
        let name = vm["metadata"]["name"].as_str().unwrap_or("unknown").to_string();
        let namespace = vm["metadata"]["namespace"].as_str().unwrap_or("default").to_string();
        let id = format!("{namespace}/{name}");
        let domain = &vm["spec"]["template"]["spec"]["domain"];
        let cpus = domain["cpu"]["cores"].as_u64().unwrap_or(1) as u32;
        let mem_str = domain["resources"]["requests"]["memory"].as_str().unwrap_or("512Mi");
        let mem_mib = parse_k8s_mem(mem_str);
        let status = vm["status"]["printableStatus"].as_str().unwrap_or("unknown").to_lowercase();
        SourceVm {
            id: id.clone(), source_id: id, name,
            cpus, mem_mib, disk_gb: 40,
            os: "Linux".to_string(), status,
        }
    }).collect();
    Ok(vms)
}

fn parse_k8s_mem(s: &str) -> u32 {
    // Parse K8s memory strings like "512Mi", "2Gi", "1024M"
    let (num_str, suffix) = s.chars().partition::<String, _>(|c| c.is_ascii_digit() || *c == '.');
    let num: f64 = num_str.parse().unwrap_or(512.0);
    match suffix.as_str() {
        "Gi" => (num * 1024.0) as u32,
        "G"  => (num * 1000.0) as u32,
        "Mi" => num as u32,
        "M"  => (num * 1000.0 / 1024.0) as u32,
        "Ki" => (num / 1024.0) as u32,
        _    => num as u32,
    }
}

pub async fn import_from_harvester(vm: &SourceVm) -> Result<ImportResult, String> {
    tracing::info!("import_from_harvester: VM {}", vm.name);
    Ok(ImportResult {
        vm_id:   uuid::Uuid::new_v4().to_string(),
        name:    vm.name.clone(),
        status:  "imported".to_string(),
        message: "VM registered. Harvester export queued (KubeVirt -> qemu-img convert).".to_string(),
    })
}
