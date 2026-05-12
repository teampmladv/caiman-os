import React, { useMemo } from 'react'
import { Server, Cpu, MemoryStick, HardDrive, Network as NetIcon } from 'lucide-react'
import { useClusterStore } from '../store/cluster'
import { clsx } from 'clsx'
import type { Vm } from '../types'

export default function TopologyPage() {
  const snapshot = useClusterStore(s => s.snapshot)
  const selectVm = useClusterStore(s => s.selectVm)

  const nodes = snapshot?.nodes ?? []
  const vms   = snapshot?.vms   ?? []

  const vmsByNode = useMemo(() => {
    const m: Record<string, Vm[]> = {}
    for (const vm of vms) {
      const key = vm.nodeName
      if (!m[key]) m[key] = []
      m[key].push(vm)
    }
    return m
  }, [vms])

  const running = vms.filter(v => v.status === 'RUNNING').length
  const total   = vms.length
  const totalMem = vms.reduce((a, v) => a + v.memTotalMib, 0)

  return (
    <div className="flex-1 overflow-auto bg-caiman-bg">
      {/* Header */}
      <div className="px-5 py-3 border-b border-caiman-border bg-caiman-bg2 flex items-center gap-4">
        <div className="text-[11px] text-[#e8f5e9] tracking-[2px] uppercase font-mono">Cluster topology</div>
        <span className="text-[9px] text-caiman-dim">
          {nodes.length} node{nodes.length !== 1 ? 's' : ''} · {running}/{total} VMs · {Math.round(totalMem / 1024)} GiB allocated
        </span>
      </div>

      <div className="p-6">
        {/* Internet cloud */}
        <div className="flex justify-center mb-2">
          <div className="px-3 py-1 rounded-full border border-caiman-border text-[9px] text-caiman-dim tracking-[1.5px] uppercase">
            ↑ Uplink · Internet
          </div>
        </div>

        {/* Vertical line down */}
        <div className="flex justify-center">
          <div className="w-px h-6 bg-caiman-border" />
        </div>

        {/* XDP gateway */}
        <div className="flex justify-center mb-2">
          <div className="px-3 py-1.5 rounded border border-caiman-border2 bg-caiman-bg3
                          text-[9px] tracking-[1.5px] uppercase flex items-center gap-2">
            <NetIcon size={10} className="text-caiman-green" />
            <span className="text-caiman-bright">caiman-net.ko</span>
            <span className="text-caiman-dim">· XDP attached</span>
          </div>
        </div>

        <div className="flex justify-center">
          <div className="w-px h-6 bg-caiman-border" />
        </div>

        {/* Nodes row */}
        <div className="flex justify-center gap-6 flex-wrap mb-2">
          {nodes.map(node => (
            <NodeCard key={node.id} node={node} vms={vmsByNode[node.hostname] ?? []} onPick={selectVm} />
          ))}
        </div>

        {/* Empty state */}
        {nodes.length === 0 && (
          <div className="text-center py-12 text-caiman-dim text-[10px]">
            No nodes registered
          </div>
        )}

        {/* Footer legend */}
        <div className="mt-8 flex justify-center gap-6 text-[8px] text-caiman-dim tracking-[1.5px] uppercase">
          <Legend color="caiman-green"  label="Running" />
          <Legend color="caiman-amber"  label="Booting" />
          <Legend color="caiman-blue"   label="Migrating" />
          <Legend color="caiman-dim"    label="Stopped" />
          <Legend color="caiman-red"    label="Error" />
        </div>
      </div>
    </div>
  )
}

function NodeCard({ node, vms, onPick }: { node: any; vms: Vm[]; onPick: (id: string) => void }) {
  const running = vms.filter(v => v.status === 'RUNNING').length
  const memUsed = vms.reduce((a, v) => a + v.memTotalMib, 0)
  const memPct  = (memUsed / (node.memTotalMib || 1)) * 100
  const cpuPct  = node.cpuUsagePct ?? 0

  const statusColor = node.status === 'HEALTHY'
    ? 'text-caiman-green border-caiman-green/40'
    : node.status === 'HIGH_LOAD'
    ? 'text-caiman-amber border-caiman-amber/40'
    : 'text-caiman-red border-caiman-red/40'

  return (
    <div className="w-[280px] bg-caiman-bg2 border border-caiman-border rounded-lg overflow-hidden shadow-panel">
      {/* Node header */}
      <div className="px-3 py-2.5 border-b border-caiman-border bg-caiman-bg3 flex items-center gap-2">
        <Server size={12} className="text-caiman-green" />
        <span className="text-[11px] text-[#e8f5e9] font-mono flex-1 truncate">{node.hostname}</span>
        <span className={clsx("px-1.5 py-0.5 text-[8px] tracking-[1.5px] uppercase rounded border", statusColor)}>
          {node.status}
        </span>
      </div>

      {/* Stats */}
      <div className="px-3 py-2 grid grid-cols-3 gap-2 text-[9px] border-b border-caiman-border">
        <Stat icon={Cpu}        label="CPU" value={`${node.cpuCores}c`}                            sub={`${cpuPct.toFixed(0)}%`} />
        <Stat icon={MemoryStick} label="RAM" value={`${Math.round(node.memTotalMib / 1024)}G`}      sub={`${memPct.toFixed(0)}%`} />
        <Stat icon={HardDrive}   label="VMs" value={`${vms.length}`}                                sub={`${running} up`} />
      </div>

      {/* VMs hosted */}
      <div className="px-3 py-2">
        <div className="text-[8px] text-caiman-dim tracking-[1.5px] uppercase mb-1.5">Virtual machines</div>
        {vms.length === 0 && (
          <div className="text-[9px] text-caiman-dim/60 italic py-2 text-center">No VMs on this node</div>
        )}
        <div className="flex flex-col gap-1">
          {vms.map(vm => {
            const sColor = vm.status === 'RUNNING'   ? 'bg-caiman-green'
                         : vm.status === 'BOOTING'   ? 'bg-caiman-amber animate-pulse'
                         : vm.status === 'MIGRATING' ? 'bg-caiman-blue animate-pulse'
                         : vm.status === 'ERROR'     ? 'bg-caiman-red'
                         : 'bg-caiman-dim'
            return (
              <button
                key={vm.id}
                onClick={() => onPick(vm.id)}
                className="flex items-center gap-2 px-2 py-1 rounded text-left bg-caiman-bg border border-caiman-border hover:border-caiman-green/50 hover:bg-caiman-bg3 transition-colors group"
              >
                <span className={clsx("w-1.5 h-1.5 rounded-full flex-shrink-0", sColor)} />
                <span className="text-[10px] font-mono text-caiman-text group-hover:text-[#e8f5e9] flex-1 truncate">{vm.name}</span>
                <span className="text-[8px] text-caiman-dim font-mono">
                  {vm.cpuCores}c·{Math.round(vm.memTotalMib / 1024)}G
                </span>
              </button>
            )
          })}
        </div>
      </div>

      {/* Footer info */}
      <div className="px-3 py-1.5 border-t border-caiman-border bg-caiman-bg3 text-[8px] text-caiman-dim tracking-[1.5px] flex items-center justify-between">
        <span>{node.kernelVersion ?? '—'}</span>
        <span>caiman {node.caimanVersion ?? '0.1.0'}</span>
      </div>
    </div>
  )
}

function Stat({ icon: Icon, label, value, sub }: any) {
  return (
    <div className="flex flex-col">
      <div className="flex items-center gap-1 text-caiman-dim mb-0.5">
        <Icon size={8} />
        <span className="text-[7px] uppercase tracking-[1px]">{label}</span>
      </div>
      <div className="text-caiman-text font-mono text-[11px]">{value}</div>
      <div className="text-[8px] text-caiman-dim">{sub}</div>
    </div>
  )
}

function Legend({ color, label }: { color: string; label: string }) {
  return (
    <span className="flex items-center gap-1.5">
      <span className={`w-1.5 h-1.5 rounded-full bg-${color}`} />
      {label}
    </span>
  )
}
