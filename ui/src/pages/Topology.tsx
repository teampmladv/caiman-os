// Topology, VMs, DRS, Microseg, Storage, GPU pages
import React from 'react'
import { useClusterStore } from '../store/cluster'
import { VmRow } from '../components/vm/VmRow'

// ── Topology page (React Flow) ────────────────────────────────────────────
export default function TopologyPage() {
  return (
    <div className="flex-1 flex items-center justify-center flex-col gap-3 text-caiman-dim">
      <div className="text-[32px] opacity-20">◎</div>
      <div className="text-[10px] tracking-[3px] uppercase">Live Topology</div>
      <div className="text-[9px] text-caiman-dim max-w-xs text-center">
        React Flow graph with node→VM→policy overlay.<br/>
        Install dependencies: <code className="text-caiman-green">npm install</code> then run dev.
      </div>
    </div>
  )
}
