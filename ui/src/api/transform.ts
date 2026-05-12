import type { ClusterSnapshot, Vm, ClusterNode, VmStatus } from '../types'

export function apiVmToUi(v: any): Vm {
  const statusMap: Record<string, VmStatus> = {
    ACTIVE:    'RUNNING',
    SHUT_OFF:  'STOPPED',
    BOOTING:   'BOOTING',
    ERROR:     'ERROR',
    MIGRATING: 'MIGRATING',
  }
  return {
    id:           v.id,
    name:         v.name,
    status:       statusMap[v.status] ?? 'STOPPED',
    nodeId:       v.nodeName,
    nodeName:     v.nodeName,
    cpuCores:     v.cpus ?? 1,
    cpuUsagePct:  v.cpuUsagePct ?? 0,
    memMib:       v.memUsedMib ?? 0,
    memTotalMib:  v.memMib ?? 256,
    diskReadIops:  0,
    diskWriteIops: 0,
    netRxMbps:    v.netRxMbps ?? 0,
    netTxMbps:    v.netTxMbps ?? 0,
    netRxDrops:   0,
    uptimeSecs:   v.uptimeSecs ?? 0,
    mac:          v.mac ?? '',
    labels:       v.labels ?? {},
    startedAt:    v.startedAt ?? v.createdAt ?? new Date().toISOString(),
    gpuAlloc:     undefined,
    migrating:    undefined,
  }
}

export function buildSnapshot(vms: any[], nodes: any[]): ClusterSnapshot {
  const uiVms = vms.map(apiVmToUi)
  const running = uiVms.filter(v => v.status === 'RUNNING')

  const uiNodes: ClusterNode[] = [{
    id:              'caiman-bare-01',
    hostname:        'caiman-bare-01',
    status:          'HEALTHY',
    cpuCores:        12,
    cpuUsagePct:     0,
    memTotalMib:     65536,
    memUsedMib:      uiVms.reduce((a, v) => a + v.memTotalMib, 0),
    diskReadIops:    0,
    diskWriteIops:   0,
    netRxMbps:       0,
    netTxMbps:       0,
    loadScore:       0,
    vmCount:         uiVms.length,
    vms:             uiVms.map(v => v.id),
    gpus:            [],
    uptime:          0,
    kernelVersion:   '6.6.69-0-virt',
    caimanVersion:   '1.3.0',
  }]

  return {
    vms:               uiVms,
    nodes:             uiNodes,
    totalCpuPct:       running.length,
    totalMemUsed:      uiVms.reduce((a, v) => a + v.memTotalMib, 0),
    totalMemMib:       65536,
    xdpThroughputGbps: 0,
    xdpDropsTotal:     0,
    balanceSigma:      0,
    drsMode:           'FullyAutomated',
    timestamp:         Date.now(),
  }
}
