import React, { useMemo } from 'react'
import { motion } from 'framer-motion'
import { useClusterStore } from '../store/cluster'
import { KpiCard } from '../components/dashboard/KpiCard'
import { NodeCard } from '../components/nodes/NodeCard'
import { VmRow } from '../components/vm/VmRow'
import { DrsPanel } from '../components/drs/DrsPanel'
import { MicrosegPanel } from '../components/microseg/MicrosegPanel'
import { ActivityFeed } from '../components/dashboard/ActivityFeed'

const FADE = (i: number) => ({
  initial: { opacity: 0, y: 8 },
  animate: { opacity: 1, y: 0 },
  transition: { delay: i * 0.05, duration: 0.2 },
})

export default function OverviewPage() {
  const { snapshot, drsRecs, auditEvents, selectVm } = useClusterStore(s => ({
    snapshot:    s.snapshot,
    drsRecs:     s.drsRecs,
    auditEvents: s.auditEvents,
    selectVm:    s.selectVm,
  }))

  if (!snapshot) {
    return (
      <div className="flex-1 flex items-center justify-center text-caiman-dim">
        <div className="flex flex-col items-center gap-3">
          <div className="w-8 h-8 rounded-full border-2 border-caiman-green border-t-transparent animate-spin" />
          <span className="text-[10px] tracking-[3px] uppercase">Connecting to cluster…</span>
        </div>
      </div>
    )
  }

  const runningVms = snapshot.vms.filter(v => v.status === 'RUNNING')
  const imbalanced = snapshot.balanceSigma > 0.10
  const deniesLast60 = auditEvents.filter(e => e.verdict === 'DENY').length

  return (
    <div className="flex-1 overflow-y-auto p-2.5 flex flex-col gap-2.5 min-h-0">

      {/* ── KPIs ─────────────────────────────────────────────────────── */}
      <div className="grid grid-cols-4 gap-2">
        {[
          {
            label: 'Cluster CPU',
            value: snapshot.totalCpuPct.toFixed(0),
            unit: '%',
            sub: `${snapshot.nodes.length} nodes · ${snapshot.nodes.reduce((s, n) => s + n.cpuCores, 0)} cores`,
            variant: snapshot.totalCpuPct > 75 ? 'warn' as const : 'ok' as const,
            delta: '+2%',
            deltaUp: true,
          },
          {
            label: 'Memory used',
            value: Math.round(snapshot.totalMemUsed / 1024).toString(),
            unit: 'GiB',
            sub: `of ${Math.round(snapshot.totalMemMib / 1024)} GiB · ${Math.round(snapshot.totalMemUsed / snapshot.totalMemMib * 100)}%`,
            variant: 'info' as const,
            delta: `+8G`,
            deltaUp: true,
          },
          {
            label: 'XDP throughput',
            value: snapshot.xdpThroughputGbps.toFixed(0),
            unit: 'Gbps',
            sub: `zero-copy · ${snapshot.xdpDropsTotal} drops`,
            variant: 'ok' as const,
            delta: 'live',
            deltaUp: true,
          },
          {
            label: 'DRS balance σ',
            value: snapshot.balanceSigma.toFixed(3),
            unit: '',
            sub: `threshold 0.10 · ${drsRecs.length} recs pending`,
            variant: imbalanced ? 'warn' as const : 'ok' as const,
            delta: imbalanced ? '+0.04' : 'OK',
            deltaUp: !imbalanced,
          },
        ].map((kpi, i) => (
          <motion.div key={kpi.label} {...FADE(i)}>
            <KpiCard {...kpi} />
          </motion.div>
        ))}
      </div>

      {/* ── Main grid ────────────────────────────────────────────────── */}
      <div className="grid grid-cols-3 gap-2.5 flex-1 min-h-0">

        {/* Nodes (2/3 width) */}
        <motion.div className="col-span-2 flex flex-col gap-2" {...FADE(4)}>
          <div className="sec-label">Nodes</div>
          <div className="grid grid-cols-3 gap-2">
            {snapshot.nodes.map(node => (
              <NodeCard key={node.id} node={node} />
            ))}
          </div>

          {/* VM table */}
          <div className="panel flex-1 min-h-0">
            <div className="panel-hd">
              <span className="panel-title">Virtual machines</span>
              <div className="flex items-center gap-2">
                <span className="text-[9px] text-caiman-bright">
                  {runningVms.length} running
                </span>
                <button className="btn-primary text-[8px] px-2 py-0.5">
                  + New VM
                </button>
              </div>
            </div>
            {/* Column headers */}
            <div className="grid gap-2 px-2.5 py-1 border-b border-caiman-border
                            text-[8px] text-caiman-dim tracking-[1.5px] uppercase"
                 style={{ gridTemplateColumns: '1fr 70px 80px 80px 72px 50px' }}>
              <span>VM</span>
              <span>Status</span>
              <span className="text-right">CPU</span>
              <span className="text-right">Memory</span>
              <span>Network RX</span>
              <span>Node</span>
            </div>
            <div className="panel-body p-1 overflow-y-auto">
              {snapshot.vms.map(vm => (
                <VmRow key={vm.id} vm={vm} onClick={() => selectVm(vm.id)} />
              ))}
            </div>
          </div>
        </motion.div>

        {/* Right column */}
        <motion.div className="flex flex-col gap-2" {...FADE(5)}>
          <DrsPanel recs={drsRecs} sigma={snapshot.balanceSigma} mode={snapshot.drsMode} />
          <MicrosegPanel events={auditEvents.slice(0, 8)} deniesTotal={deniesLast60} />
          <ActivityFeed />
        </motion.div>
      </div>
    </div>
  )
}
