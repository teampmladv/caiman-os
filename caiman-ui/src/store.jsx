import { useState, useEffect, createContext, useContext } from 'react'
import { getActiveCluster } from './components/clusters/ClusterStore.js'

export const AppContext = createContext(null)
export function useApp() { return useContext(AppContext) }

// ── Mock data (fallback when no cluster connected) ────────────────────────
export const INITIAL_NODES = [
  { id: 'n1', name: 'caiman-bare-01', ip: '65.109.83.244', cpu: 34, mem: 58, cores: 12, ram: 64, vms: 3, status: 'HEALTHY', gpu: true,  uptime: '14d 3h' },
  { id: 'n2', name: 'caiman-node-02', ip: '10.0.0.2',      cpu: 22, mem: 41, cores: 6,  ram: 24, vms: 2, status: 'HEALTHY', gpu: false, uptime: '14d 3h' },
  { id: 'n3', name: 'caiman-node-03', ip: '10.0.0.3',      cpu: 48, mem: 67, cores: 6,  ram: 24, vms: 2, status: 'HEALTHY', gpu: false, uptime: '12d 1h' },
]

export const INITIAL_VMS = [
  { id: 'vm-a1b2', name: 'nginx-prod',   cpus: 2, mem: 512,  status: 'RUNNING', cpu: 12, node: 'n1', ip: '10.10.0.1', uptime: '3d 12h', tenant: 'prod', gpu: false },
  { id: 'vm-c3d4', name: 'postgres-01',  cpus: 4, mem: 2048, status: 'RUNNING', cpu: 31, node: 'n1', ip: '10.10.0.2', uptime: '3d 12h', tenant: 'prod', gpu: false },
  { id: 'vm-e5f6', name: 'redis-cache',  cpus: 1, mem: 512,  status: 'RUNNING', cpu: 4,  node: 'n1', ip: '10.10.0.3', uptime: '2d 8h',  tenant: 'prod', gpu: false },
  { id: 'vm-g7h8', name: 'worker-01',    cpus: 2, mem: 1024, status: 'RUNNING', cpu: 67, node: 'n2', ip: '10.10.0.4', uptime: '1d 4h',  tenant: 'dev',  gpu: false },
  { id: 'vm-i9j0', name: 'worker-02',    cpus: 2, mem: 1024, status: 'RUNNING', cpu: 55, node: 'n2', ip: '10.10.0.5', uptime: '1d 4h',  tenant: 'dev',  gpu: false },
  { id: 'vm-k1l2', name: 'gpu-train-01', cpus: 8, mem: 8192, status: 'RUNNING', cpu: 89, node: 'n1', ip: '10.10.0.6', uptime: '6h 22m', tenant: 'ml',   gpu: true  },
  { id: 'vm-m3n4', name: 'monitor-01',   cpus: 1, mem: 256,  status: 'RUNNING', cpu: 8,  node: 'n3', ip: '10.10.0.7', uptime: '5d 2h',  tenant: 'ops',  gpu: false },
  { id: 'vm-o5p6', name: 'api-backup',   cpus: 1, mem: 512,  status: 'STOPPED', cpu: 0,  node: 'n3', ip: '10.10.0.8', uptime: '-',      tenant: 'prod', gpu: false },
]

export const INITIAL_SNAPSHOTS = [
  { id: 'snap-001', vm: 'postgres-01', vmId: 'vm-c3d4', created: '2026-05-04 14:30', size: '2.1 GB', status: 'OK', desc: 'Pre-migration backup' },
  { id: 'snap-002', vm: 'nginx-prod',  vmId: 'vm-a1b2', created: '2026-05-03 09:00', size: '800 MB', status: 'OK', desc: 'Weekly snapshot' },
  { id: 'snap-003', vm: 'worker-01',   vmId: 'vm-g7h8', created: '2026-05-02 22:15', size: '1.4 GB', status: 'OK', desc: 'Auto-snapshot' },
]

export const INITIAL_ALERTS = [
  { id: 'al-1', name: 'CPU High',     metric: 'cpu',    threshold: 80, op: '>', severity: 'warn', active: true,  fired: 1 },
  { id: 'al-2', name: 'RAM Critical', metric: 'mem',    threshold: 90, op: '>', severity: 'crit', active: true,  fired: 0 },
  { id: 'al-3', name: 'VM Down',      metric: 'status', threshold: 0,  op: '=', severity: 'crit', active: true,  fired: 0 },
  { id: 'al-4', name: 'Disk Usage',   metric: 'disk',   threshold: 85, op: '>', severity: 'warn', active: false, fired: 0 },
]

export const INITIAL_APIKEYS = [
  { id: 'key-1', name: 'CI/CD Pipeline', prefix: 'caim_ci_', created: '2026-04-01', last: '2026-05-05', perms: ['vms:read','vms:create'], active: true },
  { id: 'key-2', name: 'Monitoring Bot', prefix: 'caim_mo_', created: '2026-03-15', last: '2026-05-05', perms: ['vms:read','nodes:read'],  active: true },
  { id: 'key-3', name: 'Backup Agent',   prefix: 'caim_bk_', created: '2026-02-10', last: '2026-05-04', perms: ['snapshots:*','vms:read'], active: true },
  { id: 'key-4', name: 'Old Key',        prefix: 'caim_ol_', created: '2025-11-01', last: '2026-01-10', perms: ['vms:*'],                  active: false },
]

export const INITIAL_TENANTS = [
  { id: 't1', name: 'prod', color: '#22c55e', vms: 4, vcpus: 15, ram: 11264, quota_vcpus: 32, quota_ram: 32768, members: 5, role: 'admin'  },
  { id: 't2', name: 'dev',  color: '#60a5fa', vms: 2, vcpus: 4,  ram: 2048,  quota_vcpus: 16, quota_ram: 16384, members: 8, role: 'viewer' },
  { id: 't3', name: 'ml',   color: '#a78bfa', vms: 1, vcpus: 8,  ram: 8192,  quota_vcpus: 16, quota_ram: 32768, members: 3, role: 'admin'  },
  { id: 't4', name: 'ops',  color: '#fbbf24', vms: 1, vcpus: 1,  ram: 256,   quota_vcpus: 8,  quota_ram: 8192,  members: 2, role: 'admin'  },
]

export const INITIAL_BACKUPS = [
  { id: 'bk-1', name: 'Daily Full',     schedule: '0 2 * * *',   retention: '7d',  last: '2026-05-05 02:00', status: 'OK',     vms: ['postgres-01','nginx-prod'] },
  { id: 'bk-2', name: 'Weekly Offsite', schedule: '0 3 * * 0',   retention: '30d', last: '2026-04-28 03:00', status: 'OK',     vms: ['postgres-01'] },
  { id: 'bk-3', name: 'Hourly Snap',    schedule: '0 * * * *',   retention: '24h', last: '2026-05-05 17:00', status: 'RUNNING',vms: ['postgres-01','redis-cache'] },
  { id: 'bk-4', name: 'ML Checkpoint',  schedule: '0 */6 * * *', retention: '48h', last: '2026-05-05 12:00', status: 'WARN',   vms: ['gpu-train-01'] },
]

export const INITIAL_GPUS = [
  { id: 'gpu-1', node: 'caiman-bare-01', model: 'NVIDIA RTX 4090', vram: 24, usedVram: 18, temp: 72, util: 89, assignedTo: 'gpu-train-01', driver: '550.67' },
]

export const SEG_RULES = [
  { id: 'r1', name: 'web -> db',        proto: 'TCP', port: '5432',   action: 'ALLOW', lat: '5us' },
  { id: 'r2', name: 'ext -> web',       proto: 'TCP', port: '80,443', action: 'ALLOW', lat: '8us' },
  { id: 'r3', name: '* -> admin',       proto: 'ANY', port: '*',      action: 'DENY',  lat: '-'   },
  { id: 'r4', name: 'worker -> redis',  proto: 'TCP', port: '6379',   action: 'ALLOW', lat: '3us' },
  { id: 'r5', name: '* -> postgres ssh',proto: 'TCP', port: '22',     action: 'DENY',  lat: '-'   },
]

export const DRS_RULES = [
  { id: 'd1', type: 'affinity',      vms: ['nginx-prod','redis-cache'], constraint: 'SAME_NODE',    active: true  },
  { id: 'd2', type: 'anti-affinity', vms: ['postgres-01','worker-01'],  constraint: 'DIFF_NODE',    active: true  },
  { id: 'd3', type: 'cpu-limit',     vms: ['gpu-train-01'],             constraint: 'MAX_CPU_90',   active: true  },
  { id: 'd4', type: 'migration',     vms: [],                           constraint: 'AUTO_BALANCE', active: false },
]

// ── API helper ────────────────────────────────────────────────────────────
async function clusterFetch(path) {
  const cluster = getActiveCluster()
  if (!cluster) return null
  try {
    const res = await fetch(`${cluster.url}${path}`, {
      headers: { Authorization: `Bearer ${cluster.token}` },
      signal: AbortSignal.timeout(8000),
    })
    if (!res.ok) return null
    return res.json()
  } catch { return null }
}

// ── Normalize API node to store format ────────────────────────────────────
function normalizeNode(n, idx) {
  return {
    id:     n.id       || `n${idx+1}`,
    name:   n.hostname || n.name || `node-${idx+1}`,
    ip:     n.ip       || '—',
    cpu:    n.cpu_usage_pct   || n.cpu || 0,
    mem:    n.mem_usage_pct   || n.mem || 0,
    cores:  n.cpu_cores       || n.cores || 1,
    ram:    Math.round((n.mem_total_mib || 0) / 1024) || n.ram || 0,
    vms:    n.vm_count        || n.vms || 0,
    status: 'HEALTHY',
    gpu:    n.gpu || false,
    uptime: n.uptime || '—',
  }
}

// ── Normalize API VM to store format ─────────────────────────────────────
function normalizeVm(v) {
  return {
    ...v,
    cpu:    v.cpu_usage_pct || v.cpu || 0,
    mem:    v.mem_mib       || v.mem || 256,
    node:   v.node_name     || v.node || '—',
    ip:     v.ip            || '—',
    tenant: v.tenant        || 'default',
    uptime: v.uptime_secs   ? `${Math.floor(v.uptime_secs/3600)}h` : '—',
  }
}

// ── AppProvider ───────────────────────────────────────────────────────────
export function AppProvider({ children }) {
  const [nodes, setNodes]         = useState(INITIAL_NODES)
  const [vms, setVms]             = useState(INITIAL_VMS)
  const [snapshots, setSnaps]     = useState(INITIAL_SNAPSHOTS)
  const [tenants]                 = useState(INITIAL_TENANTS)
  const [alerts, setAlerts]       = useState(INITIAL_ALERTS)
  const [apiKeys, setApiKeys]     = useState(INITIAL_APIKEYS)
  const [backups]                 = useState(INITIAL_BACKUPS)
  const [gpus, setGpus]           = useState(INITIAL_GPUS)
  const [view, setView]           = useState('dashboard')
  const [toast, setToast]         = useState(null)
  const [migrating, setMigrating] = useState(null)
  const [isLive, setIsLive]       = useState(false)

  const showToast = (msg, type = 'success') => {
    setToast({ msg, type })
    setTimeout(() => setToast(null), 3000)
  }

  // ── Auto-refresh JWT token before expiry ───────────────────────────────
  const refreshToken = async () => {
    const cluster = getActiveCluster()
    if (!cluster) return
    try {
      const res = await fetch(`${cluster.url}/auth/refresh`, {
        method: 'POST',
        headers: { Authorization: `Bearer ${cluster.token}` },
      })
      if (res.ok) {
        const data = await res.json()
        const clusters = loadClusters()
        saveClusters(clusters.map(c => c.id === cluster.id ? { ...c, token: data.token } : c))
      }
    } catch {}
  }

  useEffect(() => {
    const interval = setInterval(refreshToken, 1000 * 60 * 60) // cada hora
    return () => clearInterval(interval)
  }, [])

  // ── Fetch real data from active cluster ──────────────────────────────
  const fetchClusterData = async () => {
    const cluster = getActiveCluster()
    if (!cluster) { setIsLive(false); return }

    const [clusterData, vmsData] = await Promise.all([
      clusterFetch('/api/cluster'),
      clusterFetch('/api/vms'),
    ])

    if (clusterData?.nodes) {
      setNodes(clusterData.nodes.map(normalizeNode))
      setIsLive(true)
    }
    if (Array.isArray(vmsData)) {
      setVms(vmsData.map(normalizeVm))
      setIsLive(true)
    }
  }

  useEffect(() => {
    fetchClusterData()
    const interval = setInterval(fetchClusterData, 5000)
    const handler = () => fetchClusterData()
    window.addEventListener('caiman:cluster-changed', handler)
    return () => { clearInterval(interval); window.removeEventListener('caiman:cluster-changed', handler) }
  }, [])

  // ── Simulate metrics when using mock data ────────────────────────────
  useEffect(() => {
    if (isLive) return
    const t = setInterval(() => {
      setNodes(prev => prev.map(n => ({ ...n, cpu: Math.max(5, Math.min(92, n.cpu + (Math.random()-.5)*6)), mem: Math.max(20, Math.min(92, n.mem + (Math.random()-.5)*3)) })))
      setVms(prev => prev.map(v => v.status === 'RUNNING' ? { ...v, cpu: Math.max(1, Math.min(98, v.cpu + (Math.random()-.5)*8)) } : v))
      setGpus(prev => prev.map(g => ({ ...g, util: Math.max(60, Math.min(99, g.util + (Math.random()-.5)*5)), temp: Math.max(65, Math.min(85, g.temp + (Math.random()-.5)*2)) })))
    }, 3000)
    return () => clearInterval(t)
  }, [isLive])

  // ── VM actions ────────────────────────────────────────────────────────
  const deleteVM = (id) => { setVms(prev => prev.filter(v => v.id !== id)); showToast('VM deleted') }
  const stopVM   = (id) => { setVms(prev => prev.map(v => v.id === id ? { ...v, status: 'STOPPED', cpu: 0 } : v)); showToast('VM stopped') }
  const startVM  = (id) => {
    setVms(prev => prev.map(v => v.id === id ? { ...v, status: 'BOOTING' } : v))
    setTimeout(() => setVms(prev => prev.map(v => v.id === id ? { ...v, status: 'RUNNING', cpu: Math.round(Math.random()*20+5) } : v)), 2000)
    showToast('VM starting...')
  }
  const migrateVM = (vmId, targetNodeId) => {
    setMigrating(vmId)
    showToast('Live migration started...', 'info')
    setTimeout(() => {
      setVms(prev => prev.map(v => v.id === vmId ? { ...v, node: targetNodeId } : v))
      setMigrating(null)
      showToast('Migration complete')
    }, 3000)
  }
  const createSnapshot = (vmId) => {
    const vm = vms.find(v => v.id === vmId)
    if (!vm) return
    const snap = { id: 'snap-' + Date.now(), vm: vm.name, vmId, created: new Date().toISOString().slice(0,16).replace('T',' '), size: (Math.random()*3+0.5).toFixed(1)+' GB', status: 'OK', desc: 'Manual snapshot' }
    setSnaps(prev => [snap, ...prev])
    showToast('Snapshot created: ' + vm.name)
  }
  const deleteSnapshot = (id) => { setSnaps(prev => prev.filter(s => s.id !== id)); showToast('Snapshot deleted') }
  const revokeKey      = (id) => { setApiKeys(prev => prev.map(k => k.id === id ? { ...k, active: false } : k)); showToast('API key revoked') }
  const toggleAlert    = (id) => setAlerts(prev => prev.map(a => a.id === id ? { ...a, active: !a.active } : a))

  return (
    <AppContext.Provider value={{
      nodes, vms, snapshots, tenants, alerts, apiKeys, backups, gpus,
      view, setView, toast, showToast, isLive,
      deleteVM, stopVM, startVM, migrateVM, createSnapshot,
      deleteSnapshot, revokeKey, toggleAlert, migrating,
      segRules: SEG_RULES, drsRules: DRS_RULES,
      refreshData: fetchClusterData,
    }}>
      {children}
    </AppContext.Provider>
  )
}
