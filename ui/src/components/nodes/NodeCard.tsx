import React from 'react'
import { clsx } from 'clsx'
import type { ClusterNode } from '../../types'

function miniBar(pct: number, cls: string) {
  return (
    <div className="bar-track flex-1">
      <div className={`bar-fill ${cls}`} style={{ width: `${pct}%` }} />
    </div>
  )
}

export function NodeCard({ node }: { node: ClusterNode }) {
  const memPct  = Math.round(node.memUsedMib / node.memTotalMib * 100)
  const cpuC    = node.cpuUsagePct > 80 ? 'bar-red' : node.cpuUsagePct > 60 ? 'bar-amber' : 'bar-green'
  const memC    = memPct > 85 ? 'bar-red' : memPct > 70 ? 'bar-amber' : 'bar-green'
  const isWarn  = node.status === 'HIGH_LOAD' || node.status === 'CRITICAL'
  const netMbps = node.netRxMbps / 1000
  const netPct  = Math.min(100, (netMbps / 40) * 100)

  return (
    <div className={clsx(
      'bg-caiman-bg4 border rounded-lg p-3 cursor-pointer transition-colors',
      isWarn ? 'border-caiman-amber' : 'border-caiman-border hover:border-caiman-border2',
    )}>
      <div className="flex items-center justify-between mb-2">
        <span className="text-[11px] text-[#e8f5e9] font-medium">{node.hostname}</span>
        <span className={clsx(
          'text-[8px] px-1.5 py-0.5 rounded tracking-wide border',
          isWarn
            ? 'bg-[#1a1000] text-caiman-amber border-[#f57f17]'
            : 'bg-[#0d2e0d] text-caiman-bright border-caiman-border2',
        )}>
          {node.status.replace('_', ' ')}
        </span>
      </div>
      {[
        { label: 'CPU', val: `${node.cpuUsagePct.toFixed(0)}%`, pct: node.cpuUsagePct, cls: cpuC },
        { label: 'RAM', val: `${memPct}%`,                       pct: memPct,           cls: memC },
        { label: 'NET', val: `${netMbps.toFixed(0)}G`,           pct: netPct,           cls: 'bar-bright' },
      ].map(row => (
        <div key={row.label} className="mb-1.5 last:mb-0">
          <div className="flex justify-between text-[8px] text-caiman-dim mb-0.5">
            <span>{row.label}</span><span>{row.val}</span>
          </div>
          <div className="mini-bar">
            {miniBar(row.pct, row.cls)}
          </div>
        </div>
      ))}
      <div className="text-[8px] text-caiman-dim mt-2">
        <span className="text-caiman-muted">{node.vmCount}</span> VMs ·{' '}
        {node.cpuCores}c / {Math.round(node.memTotalMib / 1024)}GiB
      </div>
    </div>
  )
}
