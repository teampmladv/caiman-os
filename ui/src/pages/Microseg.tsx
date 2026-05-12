import React, { useMemo } from 'react'
import { Shield, ShieldCheck, Activity, Info, Network as NetIcon, Tag, Sparkles } from 'lucide-react'
import { useClusterStore } from '../store/cluster'
import { clsx } from 'clsx'

export default function MicrosegPage() {
  const snapshot = useClusterStore(s => s.snapshot)
  const vms = snapshot?.vms ?? []

  // Group VMs by labels to show "identities"
  const identities = useMemo(() => {
    const groups: Record<string, { labels: Record<string,string>; vms: string[] }> = {}
    for (const vm of vms) {
      const labels = vm.labels || {}
      const key = Object.entries(labels).sort().map(([k,v]) => `${k}=${v}`).join(',') || 'no-labels'
      if (!groups[key]) groups[key] = { labels, vms: [] }
      groups[key].vms.push(vm.name)
    }
    return Object.entries(groups).map(([key, g]) => ({ key, ...g }))
  }, [vms])

  const xdpAttached = false // would come from /api/microseg/status

  return (
    <div className="flex-1 overflow-auto bg-caiman-bg">
      <div className="px-5 py-3 border-b border-caiman-border bg-caiman-bg2 flex items-center gap-4">
        <Shield size={13} className="text-caiman-green" />
        <div className="text-[11px] text-[#e8f5e9] tracking-[2px] uppercase font-mono">Micro-segmentation</div>
        <span className="text-[9px] text-caiman-dim">
          {identities.length} identit{identities.length !== 1 ? 'ies' : 'y'} · 0 policies
        </span>
      </div>

      <div className="p-6 max-w-[1100px] mx-auto space-y-6">
        {/* Status banner */}
        <div className="rounded-lg border border-caiman-amber/40 bg-amber-900/10 p-4 flex items-start gap-3">
          <Info size={14} className="text-caiman-amber mt-0.5 flex-shrink-0" />
          <div className="flex-1 text-[10px] leading-relaxed">
            <div className="text-caiman-amber font-mono text-[11px] mb-1">XDP enforcement not yet active</div>
            <div className="text-caiman-dim">
              Caimán's micro-segmentation enforces L3/L4 policies in XDP (kernel layer) at sub-10µs latency.
              The policy compiler (labels → BPF map entries) is implemented. The kernel module
              (<span className="text-caiman-text font-mono">caiman_net.ko</span>) is in development. Identities below
              are visible but no policies are currently enforced.
            </div>
          </div>
        </div>

        {/* Overview cards */}
        <div className="grid grid-cols-4 gap-3">
          <StatCard icon={Tag}         label="Identities"  value={identities.length}   sub="label-based" />
          <StatCard icon={Shield}      label="Policies"    value={0}                    sub="rules deployed" />
          <StatCard icon={NetIcon}     label="XDP"         value={xdpAttached ? "ON" : "OFF"} sub={xdpAttached ? "kernel attached" : "not loaded"} color={xdpAttached ? 'text-caiman-bright' : 'text-caiman-dim'} />
          <StatCard icon={Activity}    label="Deny rate"   value="—"                    sub="last 60s" />
        </div>

        {/* Identities */}
        <div className="bg-caiman-bg2 border border-caiman-border rounded-lg overflow-hidden">
          <div className="px-4 py-2.5 border-b border-caiman-border flex items-center gap-2">
            <Tag size={11} className="text-caiman-dim" />
            <span className="text-[10px] text-caiman-text tracking-[1.5px] uppercase">Workload identities</span>
            <span className="text-[8px] text-caiman-dim ml-auto">derived from VM labels</span>
          </div>
          <div className="divide-y divide-caiman-border">
            {identities.length === 0 && (
              <div className="p-6 text-center text-[10px] text-caiman-dim">
                No VMs to derive identities from. Create VMs and add labels (e.g. <span className="font-mono text-caiman-text">env=prod, app=web</span>) to define identities.
              </div>
            )}
            {identities.map(idn => (
              <div key={idn.key} className="px-4 py-3">
                <div className="flex items-start justify-between mb-2">
                  <div className="flex-1 min-w-0">
                    <div className="flex flex-wrap gap-1.5 mb-1.5">
                      {Object.keys(idn.labels).length === 0 ? (
                        <span className="px-1.5 py-0.5 text-[9px] font-mono rounded border border-caiman-border text-caiman-dim">
                          no labels
                        </span>
                      ) : (
                        Object.entries(idn.labels).map(([k,v]) => (
                          <span key={k} className="px-1.5 py-0.5 text-[9px] font-mono rounded border border-caiman-border2 text-caiman-bright bg-caiman-green/5">
                            {k}={v}
                          </span>
                        ))
                      )}
                    </div>
                    <div className="text-[9px] text-caiman-dim">
                      {idn.vms.length} VM{idn.vms.length !== 1 ? 's' : ''}: {idn.vms.slice(0, 4).join(', ')}{idn.vms.length > 4 ? `, +${idn.vms.length - 4} more` : ''}
                    </div>
                  </div>
                  <span className="px-1.5 py-0.5 text-[8px] tracking-[1.5px] uppercase rounded border border-caiman-border text-caiman-dim">
                    no policy
                  </span>
                </div>
              </div>
            ))}
          </div>
        </div>

        {/* What's coming */}
        <div>
          <div className="text-[9px] text-caiman-dim tracking-[2px] uppercase mb-2 flex items-center gap-2">
            <Sparkles size={10} /> Coming
          </div>
          <div className="grid grid-cols-2 gap-3">
            <FeatureCard
              title="L3/L4 policy enforcement"
              version="v1.5"
              desc="Allow/deny rules by identity, protocol, port. CRD-driven, hot reload."
            />
            <FeatureCard
              title="XDP kernel attachment"
              version="v1.5"
              desc="caiman_net.ko attaches to physical NIC. Sub-10µs enforcement at kernel layer."
            />
            <FeatureCard
              title="Flow logging"
              version="v1.6"
              desc="Allowed/denied flows recorded to BPF ringbuffer. Live deny stream in UI."
            />
            <FeatureCard
              title="L7-aware policies"
              version="v2.0"
              desc="HTTP method/path inspection via socket filters. Optional sidecar."
            />
          </div>
        </div>
      </div>
    </div>
  )
}

function StatCard({ icon: Icon, label, value, sub, color }: any) {
  return (
    <div className="bg-caiman-bg2 border border-caiman-border rounded p-3">
      <div className="flex items-center gap-1.5 text-caiman-dim mb-1.5">
        <Icon size={9} />
        <span className="text-[8px] uppercase tracking-[1.5px]">{label}</span>
      </div>
      <div className={`text-[20px] font-mono ${color ?? 'text-[#e8f5e9]'}`}>{value}</div>
      <div className="text-[8px] text-caiman-dim mt-0.5">{sub}</div>
    </div>
  )
}

function FeatureCard({ title, version, desc }: { title: string; version: string; desc: string }) {
  return (
    <div className="bg-caiman-bg2 border border-caiman-border rounded p-3">
      <div className="flex items-center gap-2 mb-1.5">
        <span className="text-[11px] text-[#e8f5e9] font-mono flex-1">{title}</span>
        <span className="px-1.5 py-0.5 text-[8px] tracking-[1.5px] uppercase rounded border border-caiman-amber/40 text-caiman-amber bg-amber-900/20">
          {version}
        </span>
      </div>
      <div className="text-[9px] text-caiman-dim leading-relaxed">{desc}</div>
    </div>
  )
}
