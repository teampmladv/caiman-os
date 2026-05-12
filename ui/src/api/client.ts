import axios from 'axios'
import type { ClusterSnapshot, Vm, ClusterNode, DrsRecommendation } from '../types'

// ── Axios instance ────────────────────────────────────────────────────────

export const api = axios.create({
  baseURL: import.meta.env.VITE_API_URL ?? 'http://localhost:8765',
  timeout: 10_000,
  headers: { 'Content-Type': 'application/json' },
})

// Token-based auth: read from localStorage on each request, no auto-login
api.interceptors.request.use((config) => {
  const token = localStorage.getItem('caiman_token')
  if (token) config.headers['Authorization'] = `Bearer ${token}`
  return config
})

api.interceptors.response.use(
  r => r,
  err => {
    if (err.response?.status === 401) {
      localStorage.removeItem('caiman_token')
      // Trigger full reload to show login page
      if (window.location.pathname !== '/login') {
        window.dispatchEvent(new Event('caiman:logout'))
      }
    }
    return Promise.reject(err)
  }
)

export function logout() {
  localStorage.removeItem('caiman_token')
  localStorage.removeItem('caiman_user')
  localStorage.removeItem('caiman_role')
  window.dispatchEvent(new Event('caiman:logout'))
}

// ── API functions ─────────────────────────────────────────────────────────

export const fetchCluster     = () => api.get<ClusterSnapshot>('/api/cluster').then(r => r.data)
export const fetchVms         = () => api.get<Vm[]>('/api/vms').then(r => r.data)
export const fetchNodes       = () => api.get<ClusterNode[]>('/api/nodes').then(r => r.data)
export const fetchDrsRecs     = () => api.get<DrsRecommendation[]>('/api/drs/recommendations').then(r => r.data)
export const executeMigration = (vmId: string, toNode: string) =>
  api.post(`/api/vms/${vmId}/migrate`, { toNode })
export const startVm          = (vmId: string) => api.post(`/api/vms/${vmId}/start`)
export const stopVm           = (vmId: string) => api.post(`/api/vms/${vmId}/stop`)
export const deleteVm          = (vmId: string) => api.delete(`/api/vms/${vmId}`)
export const getSerialLogs    = (vmId: string, lines = 100) =>
  api.get<string[]>(`/api/vms/${vmId}/console?lines=${lines}`).then(r => r.data)
export const fetchXdpStats    = () => api.get('/api/xdp/stats').then(r => r.data)
export const fetchMicrosegPolicies = () => api.get('/api/microseg/policies').then(r => r.data)
export const fetchAuditEvents = (limit = 100) =>
  api.get(`/api/microseg/audit?limit=${limit}`).then(r => r.data)
export const fetchVsanVolumes = () => api.get('/api/storage/vsan').then(r => r.data)
export const fetchGpuAllocs   = () => api.get('/api/gpu/allocations').then(r => r.data)

// ── Mock data (used when VITE_MOCK=true or API unreachable) ───────────────

export const MOCK_VMS: Vm[] = [
  { id:'vm-001', name:'vm-prod-web-01',      status:'RUNNING',   nodeId:'n1', nodeName:'node-01', cpuCores:4,  cpuUsagePct:14, memMib:8192,  memTotalMib:16384, netRxMbps:1.2,  netTxMbps:0.8,  diskReadIops:120,  diskWriteIops:45,  netRxDrops:0,  uptimeSecs:1051200, mac:'02:aa:bb:00:00:01', labels:{app:'web',env:'prod',tier:'frontend'}, startedAt:'2025-03-15T08:00:00Z' },
  { id:'vm-002', name:'vm-prod-web-02',      status:'RUNNING',   nodeId:'n1', nodeName:'node-01', cpuCores:4,  cpuUsagePct:18, memMib:8192,  memTotalMib:16384, netRxMbps:1.4,  netTxMbps:0.9,  diskReadIops:98,   diskWriteIops:32,  netRxDrops:0,  uptimeSecs:1051100, mac:'02:aa:bb:00:00:02', labels:{app:'web',env:'prod',tier:'frontend'}, startedAt:'2025-03-15T08:00:00Z' },
  { id:'vm-003', name:'vm-prod-backend-01',  status:'RUNNING',   nodeId:'n2', nodeName:'node-02', cpuCores:8,  cpuUsagePct:55, memMib:32768, memTotalMib:65536, netRxMbps:8.2,  netTxMbps:6.1,  diskReadIops:4200, diskWriteIops:1800,netRxDrops:0,  uptimeSecs:1040000, mac:'02:aa:bb:00:00:03', labels:{app:'backend',env:'prod'}, startedAt:'2025-03-15T11:00:00Z' },
  { id:'vm-004', name:'vm-prod-backend-02',  status:'RUNNING',   nodeId:'n2', nodeName:'node-02', cpuCores:8,  cpuUsagePct:61, memMib:32768, memTotalMib:65536, netRxMbps:7.8,  netTxMbps:5.9,  diskReadIops:3900, diskWriteIops:1600,netRxDrops:0,  uptimeSecs:780000,  mac:'02:aa:bb:00:00:04', labels:{app:'backend',env:'prod'}, startedAt:'2025-03-24T10:00:00Z' },
  { id:'vm-005', name:'vm-prod-db-primary',  status:'RUNNING',   nodeId:'n2', nodeName:'node-02', cpuCores:16, cpuUsagePct:68, memMib:131072,memTotalMib:262144,netRxMbps:14.3, netTxMbps:3.2,  diskReadIops:12000,diskWriteIops:8000, netRxDrops:0,  uptimeSecs:2592000, mac:'02:aa:bb:00:00:05', labels:{app:'postgres',env:'prod','role':'primary'}, startedAt:'2025-02-23T00:00:00Z' },
  { id:'vm-006', name:'vm-prod-db-replica',  status:'RUNNING',   nodeId:'n1', nodeName:'node-01', cpuCores:16, cpuUsagePct:12, memMib:131072,memTotalMib:262144,netRxMbps:2.1,  netTxMbps:12.8, diskReadIops:800,  diskWriteIops:200, netRxDrops:0,  uptimeSecs:2592000, mac:'02:aa:bb:00:00:06', labels:{app:'postgres',env:'prod','role':'replica'}, startedAt:'2025-02-23T00:00:00Z' },
  { id:'vm-007', name:'vm-prod-cache-01',    status:'RUNNING',   nodeId:'n3', nodeName:'node-03', cpuCores:4,  cpuUsagePct:8,  memMib:32768, memTotalMib:65536, netRxMbps:22.4, netTxMbps:19.1, diskReadIops:200,  diskWriteIops:80,  netRxDrops:0,  uptimeSecs:475200,  mac:'02:aa:bb:00:00:07', labels:{app:'redis',env:'prod'}, startedAt:'2025-04-17T08:00:00Z' },
  { id:'vm-008', name:'vm-ml-train-03',      status:'MIGRATING', nodeId:'n2', nodeName:'node-02', cpuCores:32, cpuUsagePct:91, memMib:65536, memTotalMib:131072,netRxMbps:38.2, netTxMbps:2.1,  diskReadIops:500,  diskWriteIops:100, netRxDrops:0,  uptimeSecs:108000,  mac:'02:aa:bb:00:00:08', labels:{app:'ml','gpu':'mig-3g.40gb'}, startedAt:'2025-04-27T10:00:00Z', migrating:{phase:'IterativeCopy',fromNode:'node-02',toNode:'node-03',progressPct:64,elapsedSecs:45} },
  { id:'vm-009', name:'vm-dev-build-01',     status:'STOPPED',   nodeId:'',   nodeName:'—',       cpuCores:8,  cpuUsagePct:0,  memMib:0,     memTotalMib:16384, netRxMbps:0,    netTxMbps:0,    diskReadIops:0,    diskWriteIops:0,   netRxDrops:0,  uptimeSecs:0,       mac:'02:aa:bb:00:00:09', labels:{app:'ci',env:'dev'}, startedAt:'—' },
  { id:'vm-010', name:'vm-staging-api-01',   status:'BOOTING',   nodeId:'n3', nodeName:'node-03', cpuCores:4,  cpuUsagePct:3,  memMib:4096,  memTotalMib:8192,  netRxMbps:0.1,  netTxMbps:0,    diskReadIops:20,   diskWriteIops:5,   netRxDrops:0,  uptimeSecs:12,      mac:'02:aa:bb:00:00:0a', labels:{app:'api',env:'staging'}, startedAt:'2025-04-28T...' },
]

export const MOCK_NODES: ClusterNode[] = [
  { id:'n1', hostname:'node-01', status:'HEALTHY',   cpuCores:32, cpuUsagePct:28, memTotalMib:262144, memUsedMib:118014, diskReadIops:5000, diskWriteIops:2000, netRxMbps:18000, netTxMbps:14000, loadScore:0.28, vmCount:4, vms:['vm-001','vm-002','vm-006','vm-007'], gpus:[], uptime:2592000, kernelVersion:'6.6.29', caimanVersion:'0.1.0' },
  { id:'n2', hostname:'node-02', status:'HIGH_LOAD', cpuCores:64, cpuUsagePct:72, memTotalMib:524288, memUsedMib:424936, diskReadIops:20600,diskWriteIops:11500,netRxMbps:44000, netTxMbps:17000, loadScore:0.74, vmCount:4, vms:['vm-003','vm-004','vm-005','vm-008'], gpus:[], uptime:2592000, kernelVersion:'6.6.29', caimanVersion:'0.1.0' },
  { id:'n3', hostname:'node-03', status:'HEALTHY',   cpuCores:32, cpuUsagePct:14, memTotalMib:262144, memUsedMib:102200, diskReadIops:700,  diskWriteIops:180,  netRxMbps:22000, netTxMbps:19000, loadScore:0.14, vmCount:2, vms:['vm-007','vm-010'], gpus:[], uptime:1728000, kernelVersion:'6.6.29', caimanVersion:'0.1.0' },
]

export const MOCK_SNAPSHOT: ClusterSnapshot = {
  nodes:              MOCK_NODES,
  vms:                MOCK_VMS,
  balanceSigma:       0.14,
  drsMode:            'FullyAutomated',
  timestamp:          Date.now(),
  totalCpuPct:        38,
  totalMemUsed:       497152,
  totalMemMib:        786432,
  xdpThroughputGbps:  84,
  xdpDropsTotal:      0,
}

export const MOCK_DRS_RECS: DrsRecommendation[] = [
  { vmId:'vm-008', vmName:'vm-ml-train-03',    fromNode:'node-02', toNode:'node-03', score:0.82, reason:'node-02 CPU 72% → balance Δσ=0.04', estimatedBlackoutMs:148 },
  { vmId:'vm-005', vmName:'vm-prod-db-primary', fromNode:'node-02', toNode:'node-01', score:0.61, reason:'node-02 RAM 81% → balance Δσ=0.03', estimatedBlackoutMs:312 },
]
