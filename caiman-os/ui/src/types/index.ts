// ── Core types for Caimán OS UI ──────────────────────────────────────────

export type VmStatus = 'RUNNING' | 'MIGRATING' | 'STOPPED' | 'BOOTING' | 'ERROR'
export type NodeStatus = 'HEALTHY' | 'HIGH_LOAD' | 'CRITICAL' | 'OFFLINE'
export type DrsMode = 'Manual' | 'SemiAutomated' | 'FullyAutomated'
export type PolicyAction = 'ALLOW' | 'DENY' | 'LOG'
export type GpuMode = 'passthrough' | 'mig' | 'vgpu'
export type StorageBackend = 'vsan' | 'iscsi' | 'nvmeof' | 'nfs' | 'fc' | 'local'

// ── VM ───────────────────────────────────────────────────────────────────

export interface Vm {
  id:         string
  name:       string
  status:     VmStatus
  nodeId:     string
  nodeName:   string
  cpuCores:   number
  cpuUsagePct: number
  memMib:     number
  memTotalMib: number
  diskReadIops:  number
  diskWriteIops: number
  netRxMbps:  number
  netTxMbps:  number
  netRxDrops: number
  uptimeSecs: number
  mac:        string
  labels:     Record<string, string>
  gpuAlloc?:  GpuAlloc
  startedAt:  string
  migrating?: MigrationStatus
}

export interface MigrationStatus {
  phase:         string
  fromNode:      string
  toNode:        string
  progressPct:   number
  elapsedSecs:   number
  blackoutMs?:   number
}

export interface GpuAlloc {
  mode:    GpuMode
  profile: string
  vfio:    string
  pci:     string
}

// ── Node ─────────────────────────────────────────────────────────────────

export interface ClusterNode {
  id:              string
  hostname:        string
  status:          NodeStatus
  cpuCores:        number
  cpuUsagePct:     number
  memTotalMib:     number
  memUsedMib:      number
  diskReadIops:    number
  diskWriteIops:   number
  netRxMbps:       number
  netTxMbps:       number
  loadScore:       number
  vmCount:         number
  vms:             string[]    // VM IDs
  gpus:            GpuDevice[]
  uptime:          number
  kernelVersion:   string
  caimanVersion:   string
}

export interface GpuDevice {
  pciAddress:  string
  model:       string
  vramMib:     number
  migCapable:  boolean
  vgpuCapable: boolean
  utilPct:     number
  allocations: GpuAlloc[]
}

// ── Cluster snapshot ──────────────────────────────────────────────────────

export interface ClusterSnapshot {
  nodes:        ClusterNode[]
  vms:          Vm[]
  balanceSigma: number
  drsMode:      DrsMode
  timestamp:    number
  totalCpuPct:  number
  totalMemUsed: number
  totalMemMib:  number
  xdpThroughputGbps: number
  xdpDropsTotal: number
}

// ── DRS ───────────────────────────────────────────────────────────────────

export interface DrsRecommendation {
  vmId:       string
  vmName:     string
  fromNode:   string
  toNode:     string
  score:      number
  reason:     string
  estimatedBlackoutMs: number
}

export interface AffinityRule {
  id:        string
  name:      string
  ruleType:  'Affinity' | 'AntiAffinity'
  scope:     'Hard' | 'Soft'
  vmIds:     string[]
  vmLabels:  Record<string, string>
}

export interface ResourcePool {
  name:    string
  parent?: string
  cpu:     ResourceAlloc
  memory:  ResourceAlloc
  vmCount: number
}

export interface ResourceAlloc {
  reservation: number
  limit:       number
  shares:      'Low' | 'Normal' | 'High' | number
  expandable:  boolean
  usedPct:     number
}

// ── Micro-segmentation ────────────────────────────────────────────────────

export interface MicroSegPolicy {
  name:      string
  namespace: string
  priority:  number
  action:    PolicyAction
  from:      LabelSelector[]
  to:        LabelSelector[]
  ports?:    PortRule[]
  hitCount:  number
  denyCount: number
}

export interface LabelSelector {
  matchLabels: Record<string, string>
}

export interface PortRule {
  protocol?: 'TCP' | 'UDP' | 'ICMP'
  port?:     number
}

export interface AuditEvent {
  id:          string
  timestampNs: number
  srcId:       number
  dstId:       number
  srcMac:      string
  dstMac:      string
  srcIp:       string
  dstIp:       string
  proto:       string
  dstPort:     number
  verdict:     PolicyAction
  ruleId:      number
}

// ── Storage ───────────────────────────────────────────────────────────────

export interface VsanVolume {
  id:         string
  name:       string
  sizeGib:    number
  usedGib:    number
  ftt:        number
  raidType:   'Mirroring' | 'Erasure5' | 'Erasure6'
  state:      'Healthy' | 'Degraded' | 'Offline' | 'Resyncing'
  iopsRead:   number
  iopsWrite:  number
  latencyMs:  number
  attachedVm?: string
  encrypted:  boolean
  compression: boolean
}

export interface VVol {
  id:       string
  name:     string
  sizeGib:  number
  backend:  StorageBackend
  vmId?:    string
  state:    'Attached' | 'Detached' | 'Error'
  latencyMs: number
}

// ── Notifications ─────────────────────────────────────────────────────────

export type NotifLevel = 'info' | 'success' | 'warning' | 'error'

export interface Notification {
  id:        string
  level:     NotifLevel
  title:     string
  message:   string
  timestamp: number
  read:      boolean
  action?:   { label: string; onClick: () => void }
}

// ── API responses ─────────────────────────────────────────────────────────

export interface ApiResponse<T> {
  data:      T
  timestamp: number
  ok:        boolean
  error?:    string
}

// ── WebSocket events ──────────────────────────────────────────────────────

export type WsEvent =
  | { type: 'VM_METRICS_UPDATE';  payload: Pick<Vm, 'id' | 'cpuUsagePct' | 'netRxMbps' | 'netTxMbps' | 'memMib'> }
  | { type: 'NODE_METRICS_UPDATE'; payload: Pick<ClusterNode, 'id' | 'cpuUsagePct' | 'memUsedMib' | 'loadScore'> }
  | { type: 'VM_STATUS_CHANGE';   payload: { id: string; status: VmStatus; migrating?: MigrationStatus } }
  | { type: 'DRS_RECOMMENDATION'; payload: DrsRecommendation }
  | { type: 'MICROSEG_DENY';      payload: AuditEvent }
  | { type: 'ALERT';              payload: Notification }
  | { type: 'MIGRATION_PROGRESS'; payload: { vmId: string; phase: string; progressPct: number } }
