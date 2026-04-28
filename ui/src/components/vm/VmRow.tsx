import React from 'react'
import { clsx } from 'clsx'
import type { Vm } from '../../types'

const pillMap: Record<Vm['status'], string> = {
  RUNNING:   'pill-run',
  MIGRATING: 'pill-mig',
  STOPPED:   'pill-stop',
  BOOTING:   'pill-boot',
  ERROR:     'pill-crit',
}

export function VmRow({ vm, onClick }: { vm: Vm; onClick: () => void }) {
  const memPct = Math.round(vm.memMib / vm.memTotalMib * 100)
  const cpuC   = vm.cpuUsagePct > 80 ? 'bar-red' : vm.cpuUsagePct > 60 ? 'bar-amber' : 'bar-green'
  const memC   = memPct > 85 ? 'bar-red' : memPct > 70 ? 'bar-amber' : 'bar-green'
  const hasDot = vm.status !== 'STOPPED' && vm.status !== 'ERROR'

  return (
    <div
      className="data-row cursor-pointer"
      style={{ gridTemplateColumns: '1fr 70px 80px 80px 72px 50px' }}
      onClick={onClick}
    >
      <div>
        <div className="text-[10px] text-[#e8f5e9] font-medium truncate">{vm.name}</div>
        {vm.migrating && (
          <div className="text-[8px] text-caiman-blue mt-0.5">
            {vm.migrating.fromNode} → {vm.migrating.toNode} · {vm.migrating.progressPct.toFixed(0)}%
          </div>
        )}
      </div>
      <div>
        <span className={`pill ${pillMap[vm.status]}`}>
          {hasDot && <span className="w-1 h-1 rounded-full bg-current animate-pulse-fast" />}
          {vm.status.slice(0, 3)}
        </span>
      </div>
      <div className="mini-bar justify-end">
        <div className="bar-track">
          <div className={`bar-fill ${cpuC}`} style={{ width: `${vm.cpuUsagePct}%` }} />
        </div>
        <span className={clsx(
          'text-[9px] min-w-[28px] text-right tabular-nums',
          vm.cpuUsagePct > 80 ? 'text-caiman-red' : vm.cpuUsagePct > 60 ? 'text-caiman-amber' : 'text-caiman-text'
        )}>
          {vm.cpuUsagePct.toFixed(0)}%
        </span>
      </div>
      <div className="mini-bar justify-end">
        <div className="bar-track">
          <div className={`bar-fill ${memC}`} style={{ width: `${memPct}%` }} />
        </div>
        <span className="text-[9px] min-w-[36px] text-right tabular-nums text-caiman-text">
          {Math.round(vm.memMib / 1024)}G
        </span>
      </div>
      <div className="text-[9px] text-caiman-bright tabular-nums">
        ↓{vm.netRxMbps.toFixed(1)}
      </div>
      <div className="text-[9px] text-caiman-dim truncate">{vm.nodeName}</div>
    </div>
  )
}
