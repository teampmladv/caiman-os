// ── KpiCard ───────────────────────────────────────────────────────────────
// src/components/dashboard/KpiCard.tsx
import React from 'react'
import { clsx } from 'clsx'

type Variant = 'ok' | 'warn' | 'info' | 'crit'

interface KpiCardProps {
  label:    string
  value:    string
  unit:     string
  sub:      string
  variant:  Variant
  delta:    string
  deltaUp:  boolean
}

export function KpiCard({ label, value, unit, sub, variant, delta, deltaUp }: KpiCardProps) {
  const accentColor: Record<Variant, string> = {
    ok:   'bg-caiman-bright',
    warn: 'bg-caiman-amber',
    info: 'bg-caiman-blue',
    crit: 'bg-caiman-red',
  }
  return (
    <div className="kpi-card p-3 relative">
      {/* Bottom accent bar */}
      <div className={`absolute bottom-0 left-0 right-0 h-[2px] ${accentColor[variant]}`} />

      <div className="text-[8px] text-caiman-dim tracking-[2px] uppercase mb-1">{label}</div>
      <div className="text-[24px] font-light text-[#e8f5e9] leading-none tabular-nums font-mono">
        {value}
        {unit && <span className="text-[11px] text-caiman-green ml-1">{unit}</span>}
      </div>
      <div className="text-[9px] text-caiman-dim mt-1">{sub}</div>
      <div className={clsx(
        'absolute top-2.5 right-2.5 text-[8px] px-1.5 py-0.5 rounded',
        deltaUp
          ? 'bg-[#0d2e0d] text-caiman-bright'
          : 'bg-[#1a0d0d] text-caiman-red',
      )}>
        {delta}
      </div>
    </div>
  )
}

// ── NodeCard ──────────────────────────────────────────────────────────────
// src/components/nodes/NodeCard.tsx
import type { ClusterNode } from '../../types'

export function NodeCard({ node }: { node: ClusterNode }) {
  const isWarn = node.status === 'HIGH_LOAD' || node.status === 'CRITICAL'
  const memPct = Math.round(node.memUsedMib / node.memTotalMib * 100)
  const cpuC   = node.cpuUsagePct > 80 ? 'bar-red' : node.cpuUsagePct > 60 ? 'bar-amber' : 'bar-green'
  const memC   = memPct > 85 ? 'bar-red' : memPct > 70 ? 'bar-amber' : 'bar-green'

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
        { label: 'NET', val: `${(node.netRxMbps / 1000).toFixed(0)}G`,
                                                                  pct: Math.min(100, node.netRxMbps / 400), cls: 'bar-bright' },
      ].map(row => (
        <div key={row.label} className="mb-1.5 last:mb-0">
          <div className="flex justify-between text-[8px] text-caiman-dim mb-0.5">
            <span>{row.label}</span><span>{row.val}</span>
          </div>
          <div className="bar-track">
            <div className={`bar-fill ${row.cls}`} style={{ width: `${row.pct}%` }} />
          </div>
        </div>
      ))}

      <div className="text-[8px] text-caiman-dim mt-2">
        <span className="text-caiman-muted">{node.vmCount}</span> VMs ·
        {' '}{node.cpuCores}c / {Math.round(node.memTotalMib / 1024)}GiB
      </div>
    </div>
  )
}

// ── VmRow ─────────────────────────────────────────────────────────────────
// src/components/vm/VmRow.tsx
import type { Vm } from '../../types'

export function VmRow({ vm, onClick }: { vm: Vm; onClick: () => void }) {
  const memPct = Math.round(vm.memMib / vm.memTotalMib * 100)
  const cpuC   = vm.cpuUsagePct > 80 ? 'bar-red' : vm.cpuUsagePct > 60 ? 'bar-amber' : 'bar-green'
  const memC   = memPct > 85 ? 'bar-red' : memPct > 70 ? 'bar-amber' : 'bar-green'

  const pillMap: Record<Vm['status'], string> = {
    RUNNING: 'pill-run', MIGRATING: 'pill-mig', STOPPED: 'pill-stop',
    BOOTING: 'pill-boot', ERROR: 'pill-crit',
  }
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
        <span className={clsx('text-[9px] min-w-[28px] text-right tabular-nums',
          vm.cpuUsagePct > 80 ? 'text-caiman-red' : vm.cpuUsagePct > 60 ? 'text-caiman-amber' : 'text-caiman-text')}>
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

// ── DrsPanel ──────────────────────────────────────────────────────────────
// src/components/drs/DrsPanel.tsx
import type { DrsRecommendation, DrsMode } from '../../types'

export function DrsPanel({ recs, sigma, mode }: {
  recs: DrsRecommendation[]; sigma: number; mode: DrsMode
}) {
  return (
    <div className="panel">
      <div className="panel-hd">
        <span className="panel-title">DRS recommendations</span>
        <span className={clsx('text-[9px]', sigma > 0.10 ? 'text-caiman-amber' : 'text-caiman-green')}>
          σ={sigma.toFixed(3)}
        </span>
      </div>
      <div className="panel-body">
        {recs.slice(0, 3).map((r, i) => (
          <div key={i} className="flex items-center gap-2 py-1.5 border-b border-caiman-bg last:border-b-0">
            <span className="text-[10px] text-caiman-bright font-mono min-w-[28px] text-right">
              {r.score.toFixed(2)}
            </span>
            <div className="flex-1 min-w-0">
              <div className="text-[10px] text-[#e8f5e9] truncate">{r.vmName}</div>
              <div className="text-[8px] text-caiman-blue">{r.fromNode} → {r.toNode}</div>
            </div>
            <button className="text-[8px] px-1.5 py-0.5 border border-[#1565c0]
                               rounded bg-[#0a1f2e] text-caiman-blue hover:bg-[#0d2e3e]
                               transition-colors cursor-pointer">
              RUN
            </button>
          </div>
        ))}
        {recs.length === 0 && (
          <div className="text-[9px] text-caiman-dim py-2 text-center">
            Cluster balanced ✓
          </div>
        )}
        <div className="flex justify-between items-center mt-2 pt-2 border-t border-caiman-border">
          <span className="text-[8px] text-caiman-dim">{mode}</span>
          <button className="text-[8px] text-caiman-bright hover:underline cursor-pointer">
            Execute all ↗
          </button>
        </div>
      </div>
    </div>
  )
}

// ── MicrosegPanel ─────────────────────────────────────────────────────────
// src/components/microseg/MicrosegPanel.tsx
import type { AuditEvent } from '../../types'

export function MicrosegPanel({ events, deniesTotal }: {
  events: AuditEvent[]; deniesTotal: number
}) {
  return (
    <div className="panel">
      <div className="panel-hd">
        <span className="panel-title">Micro-seg · XDP</span>
        <span className="text-[9px] text-caiman-red">{deniesTotal} denies/60s</span>
      </div>
      <div className="panel-body">
        {events.slice(0, 5).map((e, i) => (
          <div key={i} className="flex items-center gap-2 py-1 border-b border-caiman-bg last:border-b-0 text-[9px]">
            <span className={e.verdict === 'DENY'
              ? 'text-caiman-red min-w-[36px]'
              : 'text-caiman-bright min-w-[36px]'}>
              {e.verdict}
            </span>
            <span className="text-caiman-text flex-1 truncate">{e.srcIp} → {e.dstIp}</span>
            <span className="text-caiman-dim">:{e.dstPort}</span>
          </div>
        ))}
        {events.length === 0 && (
          <div className="text-[9px] text-caiman-dim py-2 text-center">No recent events</div>
        )}
      </div>
    </div>
  )
}

// ── ActivityFeed ──────────────────────────────────────────────────────────
// src/components/dashboard/ActivityFeed.tsx
import { useClusterStore as useStore } from '../../store/cluster'

export function ActivityFeed() {
  const notifs = useStore(s => s.notifications.slice(0, 8))
  const levelColor: Record<string, string> = {
    info:    'text-caiman-blue',
    success: 'text-caiman-bright',
    warning: 'text-caiman-amber',
    error:   'text-caiman-red',
  }
  return (
    <div className="panel flex-1">
      <div className="panel-hd">
        <span className="panel-title">Activity</span>
      </div>
      <div className="panel-body">
        {notifs.map(n => (
          <div key={n.id} className="flex gap-2 py-1 border-b border-caiman-bg last:border-b-0">
            <span className={`text-[9px] ${levelColor[n.level] ?? 'text-caiman-text'} flex-shrink-0`}>
              {n.level === 'warning' ? '⚠' : n.level === 'error' ? '✕' : n.level === 'success' ? '✓' : 'ℹ'}
            </span>
            <span className="text-[9px] text-caiman-text leading-relaxed">{n.message}</span>
          </div>
        ))}
        {notifs.length === 0 && (
          <div className="text-[9px] text-caiman-dim py-2 text-center">No recent activity</div>
        )}
      </div>
    </div>
  )
}

// ── NotificationStack ─────────────────────────────────────────────────────
// src/components/ui/NotificationStack.tsx
import { AnimatePresence, motion as m } from 'framer-motion'
import { X } from 'lucide-react'

export function NotificationStack() {
  const { notifications, clearNotif } = useStore(s => ({
    notifications: s.notifications.filter(n => !n.read).slice(0, 4),
    clearNotif:    s.clearNotif,
  }))
  const levelBorder: Record<string, string> = {
    info:    'border-caiman-blue',
    success: 'border-caiman-border2',
    warning: 'border-caiman-amber',
    error:   'border-caiman-red',
  }
  return (
    <div className="fixed bottom-10 right-3 flex flex-col gap-1.5 z-50 pointer-events-none">
      <AnimatePresence>
        {notifications.map(n => (
          <m.div
            key={n.id}
            initial={{ x: 60, opacity: 0 }}
            animate={{ x: 0, opacity: 1 }}
            exit={{ x: 60, opacity: 0 }}
            transition={{ type: 'spring', stiffness: 300, damping: 25 }}
            className={`pointer-events-auto flex items-center gap-2.5 pl-3 pr-2 py-2
                        bg-caiman-bg4 border rounded-lg text-[10px] text-caiman-text
                        max-w-[280px] shadow-panel ${levelBorder[n.level] ?? 'border-caiman-border'}`}
          >
            <div className={`w-1.5 h-1.5 rounded-full flex-shrink-0 ${
              n.level === 'success' ? 'bg-caiman-bright' :
              n.level === 'warning' ? 'bg-caiman-amber'  :
              n.level === 'error'   ? 'bg-caiman-red'    : 'bg-caiman-blue'
            }`} />
            <span className="flex-1 leading-relaxed">{n.message}</span>
            <button
              onClick={() => clearNotif(n.id)}
              className="text-caiman-dim hover:text-caiman-text flex-shrink-0"
            >
              <X size={10} />
            </button>
          </m.div>
        ))}
      </AnimatePresence>
    </div>
  )
}
