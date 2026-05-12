import React from 'react'
import { RefreshCw, TrendingUp, Activity, Sparkles, Server, ArrowRight, Info } from 'lucide-react'
import { useClusterStore } from '../store/cluster'

export default function DRSPage() {
  const snapshot = useClusterStore(s => s.snapshot)
  const nodes    = snapshot?.nodes ?? []
  const vms      = snapshot?.vms   ?? []

  const sigma = snapshot?.balanceSigma ?? 0
  const balanced = sigma < 0.1
  const singleNode = nodes.length < 2

  return (
    <div className="flex-1 overflow-auto bg-caiman-bg">
      {/* Header */}
      <div className="px-5 py-3 border-b border-caiman-border bg-caiman-bg2 flex items-center gap-4">
        <RefreshCw size={13} className="text-caiman-green" />
        <div className="text-[11px] text-[#e8f5e9] tracking-[2px] uppercase font-mono">Distributed Resource Scheduler</div>
        <span className="text-[9px] text-caiman-dim">
          σ={sigma.toFixed(3)} · {balanced ? 'balanced' : 'unbalanced'}
        </span>
      </div>

      <div className="p-6 max-w-[1100px] mx-auto">
        {/* Status bar */}
        <div className={`mb-6 rounded-lg border p-4 flex items-center gap-3 ${
          balanced ? 'border-caiman-green/30 bg-caiman-green/5' : 'border-caiman-amber/40 bg-amber-900/10'
        }`}>
          <div className={`w-8 h-8 rounded-full flex items-center justify-center flex-shrink-0 ${
            balanced ? 'bg-caiman-green/20' : 'bg-caiman-amber/20'
          }`}>
            {balanced
              ? <Sparkles size={14} className="text-caiman-bright" />
              : <Activity size={14} className="text-caiman-amber" />
            }
          </div>
          <div className="flex-1">
            <div className={`text-[12px] font-mono ${balanced ? 'text-caiman-bright' : 'text-caiman-amber'}`}>
              {balanced ? 'Cluster is balanced' : 'Cluster needs rebalancing'}
            </div>
            <div className="text-[10px] text-caiman-dim mt-0.5">
              {singleNode
                ? 'DRS becomes active when you have 2 or more nodes. Add nodes via federation to enable live migration and load balancing.'
                : balanced
                  ? 'No recommendations at this time. DRS is monitoring load distribution every 30s.'
                  : 'Load imbalance detected. Click "Generate plan" to see migration recommendations.'
              }
            </div>
          </div>
          {!singleNode && (
            <button className="px-3 py-1.5 rounded bg-caiman-green/15 border border-caiman-green/50 text-caiman-bright text-[10px] tracking-[2px] uppercase hover:bg-caiman-green/25 transition-all flex items-center gap-2">
              <TrendingUp size={11} /> Generate plan
            </button>
          )}
        </div>

        {/* Mode + metrics row */}
        <div className="grid grid-cols-3 gap-3 mb-6">
          <MetricCard label="Mode" value="FullyAutomated" sub="auto-execute σ>0.10" color="text-caiman-bright" />
          <MetricCard label="Balance σ" value={sigma.toFixed(3)} sub="lower is better" color={balanced ? 'text-caiman-bright' : 'text-caiman-amber'} />
          <MetricCard label="Threshold" value="0.10" sub="recommendation cutoff" color="text-caiman-text" />
        </div>

        {/* Node load distribution */}
        <div className="bg-caiman-bg2 border border-caiman-border rounded-lg overflow-hidden mb-6">
          <div className="px-4 py-2.5 border-b border-caiman-border flex items-center gap-2">
            <Server size={11} className="text-caiman-dim" />
            <span className="text-[10px] text-caiman-text tracking-[1.5px] uppercase">Node load distribution</span>
          </div>
          <div className="p-4 space-y-3">
            {nodes.length === 0 && (
              <div className="text-[10px] text-caiman-dim text-center py-3">No nodes available</div>
            )}
            {nodes.map(node => {
              const nodeVms = vms.filter(v => v.nodeName === node.hostname)
              const memUsed = nodeVms.reduce((a, v) => a + v.memTotalMib, 0)
              const memPct  = (memUsed / (node.memTotalMib || 1)) * 100
              const cpuPct  = node.cpuUsagePct ?? 0
              return (
                <div key={node.id} className="flex items-center gap-4">
                  <div className="w-32 text-[10px] font-mono text-caiman-text truncate">{node.hostname}</div>
                  <Bar  label="CPU" pct={cpuPct} />
                  <Bar  label="MEM" pct={memPct} />
                  <div className="w-12 text-[9px] font-mono text-caiman-dim text-right">{nodeVms.length} VM</div>
                </div>
              )
            })}
          </div>
        </div>

        {/* Recommendations placeholder */}
        <div className="bg-caiman-bg2 border border-caiman-border rounded-lg overflow-hidden">
          <div className="px-4 py-2.5 border-b border-caiman-border flex items-center gap-2">
            <ArrowRight size={11} className="text-caiman-dim" />
            <span className="text-[10px] text-caiman-text tracking-[1.5px] uppercase">Recommendations</span>
            <span className="text-[8px] text-caiman-dim ml-auto">{singleNode ? 'requires multi-node' : '0 pending'}</span>
          </div>
          <div className="p-4 text-[10px] text-caiman-dim text-center">
            {singleNode ? (
              <div className="flex items-start gap-3 text-left">
                <Info size={12} className="text-caiman-dim mt-0.5 flex-shrink-0" />
                <div>
                  <div className="text-caiman-text mb-1">DRS is dormant in single-node mode</div>
                  <div className="leading-relaxed">
                    To enable live migration, fault-tolerant placement, and automatic load balancing, federate this node with at least one additional Caimán host. See <span className="text-caiman-green">Settings → Federation</span> (coming soon).
                  </div>
                </div>
              </div>
            ) : balanced ? (
              <span>Cluster balanced. DRS will surface recommendations here when σ exceeds threshold.</span>
            ) : (
              <span>Click "Generate plan" to compute optimal VM placement.</span>
            )}
          </div>
        </div>
      </div>
    </div>
  )
}

function MetricCard({ label, value, sub, color }: { label: string; value: string; sub: string; color: string }) {
  return (
    <div className="bg-caiman-bg2 border border-caiman-border rounded p-3">
      <div className="text-[8px] text-caiman-dim tracking-[2px] uppercase mb-1">{label}</div>
      <div className={`text-[18px] font-mono ${color}`}>{value}</div>
      <div className="text-[8px] text-caiman-dim mt-0.5">{sub}</div>
    </div>
  )
}

function Bar({ label, pct }: { label: string; pct: number }) {
  const color = pct > 80 ? 'bg-caiman-red' : pct > 60 ? 'bg-caiman-amber' : 'bg-caiman-green'
  return (
    <div className="flex items-center gap-2 flex-1">
      <span className="text-[8px] text-caiman-dim tracking-[1.5px] w-8">{label}</span>
      <div className="flex-1 h-2 bg-caiman-bg rounded-full overflow-hidden border border-caiman-border">
        <div className={`h-full ${color} transition-all`} style={{ width: `${Math.min(pct, 100)}%` }} />
      </div>
      <span className="text-[9px] text-caiman-dim font-mono w-9 text-right">{pct.toFixed(0)}%</span>
    </div>
  )
}
