import React, { useMemo } from 'react'
import { HardDrive, Database, Layers, Lock, Sparkles } from 'lucide-react'
import { useClusterStore } from '../store/cluster'
import { clsx } from 'clsx'

export default function StoragePage() {
  const snapshot = useClusterStore(s => s.snapshot)
  const vms = snapshot?.vms ?? []

  const disks = useMemo(() => vms.map(vm => ({
    id:        vm.id,
    vmName:    vm.name,
    path:      `/var/lib/caiman/vms/${vm.id}/disk.img`,
    sizeMib:   64,
    baseImage: 'caiman-base-1.0.img',
    status:    vm.status,
  })), [vms])

  const totalSizeMib = disks.reduce((a, d) => a + d.sizeMib, 0)
  const mounted = disks.filter(d => d.status === 'RUNNING').length

  return (
    <div className="flex-1 overflow-auto bg-caiman-bg">
      <div className="px-5 py-3 border-b border-caiman-border bg-caiman-bg2 flex items-center gap-4">
        <HardDrive size={13} className="text-caiman-green" />
        <div className="text-[11px] text-[#e8f5e9] tracking-[2px] uppercase font-mono">Storage</div>
        <span className="text-[9px] text-caiman-dim">
          {disks.length} disks · {(totalSizeMib / 1024).toFixed(1)} GiB allocated · {mounted} mounted
        </span>
      </div>

      <div className="p-6 max-w-[1100px] mx-auto space-y-6">
        {/* Active backend */}
        <div>
          <div className="text-[9px] text-caiman-dim tracking-[2px] uppercase mb-2">Active backend</div>
          <BackendCard
            name="local-file"
            badge="ACTIVE"
            badgeColor="text-caiman-bright bg-caiman-green/10 border-caiman-green/40"
            desc="Per-VM raw disk images on the host filesystem"
            details="Copy-on-write clone from base image · ext4 on host · no replication"
            stats={[
              { label: 'Path',  value: '/var/lib/caiman/vms/' },
              { label: 'Disks', value: `${disks.length}` },
              { label: 'Size',  value: `${(totalSizeMib / 1024).toFixed(1)} GiB` },
            ]}
          />
        </div>

        {/* Disks table */}
        <div className="bg-caiman-bg2 border border-caiman-border rounded-lg overflow-hidden">
          <div className="px-4 py-2.5 border-b border-caiman-border flex items-center gap-2">
            <Layers size={11} className="text-caiman-dim" />
            <span className="text-[10px] text-caiman-text tracking-[1.5px] uppercase">Virtual disks</span>
            <span className="text-[8px] text-caiman-dim ml-auto">{disks.length} total</span>
          </div>
          <table className="w-full text-[10px] font-mono">
            <thead className="bg-caiman-bg3 border-b border-caiman-border">
              <tr className="text-[8px] text-caiman-dim tracking-[1.5px] uppercase">
                <th className="px-3 py-2 text-left">VM</th>
                <th className="px-3 py-2 text-left">Path</th>
                <th className="px-3 py-2 text-left">Base image</th>
                <th className="px-3 py-2 text-right">Size</th>
                <th className="px-3 py-2 text-left">Status</th>
              </tr>
            </thead>
            <tbody>
              {disks.map(d => (
                <tr key={d.id} className="border-b border-caiman-border hover:bg-caiman-bg2">
                  <td className="px-3 py-2 text-[#e8f5e9]">{d.vmName}</td>
                  <td className="px-3 py-2 text-caiman-dim truncate max-w-[280px]">{d.path}</td>
                  <td className="px-3 py-2 text-caiman-text">{d.baseImage}</td>
                  <td className="px-3 py-2 text-right text-caiman-text">{d.sizeMib} MiB</td>
                  <td className="px-3 py-2">
                    <span className={clsx(
                      "px-1.5 py-0.5 text-[8px] tracking-[1.5px] uppercase rounded border",
                      d.status === 'RUNNING' ? "border-caiman-green/40 text-caiman-bright bg-caiman-green/10" : "border-caiman-border text-caiman-dim"
                    )}>
                      {d.status === 'RUNNING' ? 'mounted' : 'idle'}
                    </span>
                  </td>
                </tr>
              ))}
              {disks.length === 0 && (
                <tr><td colSpan={5} className="text-center py-6 text-caiman-dim">No disks allocated</td></tr>
              )}
            </tbody>
          </table>
        </div>

        {/* Base images */}
        <div className="bg-caiman-bg2 border border-caiman-border rounded-lg overflow-hidden">
          <div className="px-4 py-2.5 border-b border-caiman-border flex items-center gap-2">
            <Database size={11} className="text-caiman-dim" />
            <span className="text-[10px] text-caiman-text tracking-[1.5px] uppercase">Base images</span>
          </div>
          <div className="p-4">
            <div className="bg-caiman-bg border border-caiman-border rounded p-3 flex items-center gap-3">
              <div className="w-8 h-8 rounded bg-caiman-green/10 border border-caiman-green/30 flex items-center justify-center">
                <HardDrive size={12} className="text-caiman-bright" />
              </div>
              <div className="flex-1">
                <div className="text-[11px] text-[#e8f5e9] font-mono">caiman-base-1.0.img</div>
                <div className="text-[9px] text-caiman-dim">Alpine 3.19 · busybox · ext4 · root login</div>
              </div>
              <div className="text-[10px] text-caiman-text font-mono">64 MiB</div>
              <span className="px-1.5 py-0.5 text-[8px] tracking-[1.5px] uppercase rounded border border-caiman-border2 text-caiman-bright">
                default
              </span>
            </div>
            <div className="mt-2 text-[9px] text-caiman-dim text-center">
              Multi-image library (Ubuntu, Debian, Rocky, custom ISOs) coming in v1.5
            </div>
          </div>
        </div>

        {/* Roadmap */}
        <div>
          <div className="text-[9px] text-caiman-dim tracking-[2px] uppercase mb-2 flex items-center gap-2">
            <Sparkles size={10} /> Coming soon
          </div>
          <div className="grid grid-cols-2 gap-3">
            <BackendCard
              name="local-lvm"
              badge="v1.5"
              badgeColor="text-caiman-amber bg-amber-900/20 border-caiman-amber/40"
              desc="LVM thin pool with instant snapshots"
              details="Logical volumes · thin provisioning · CoW snapshots · per-VM LV"
              stats={[]}
              compact
            />
            <BackendCard
              name="nfs"
              badge="v1.5"
              badgeColor="text-caiman-amber bg-amber-900/20 border-caiman-amber/40"
              desc="NFS-mounted shared pool"
              details="Bring your own NFS server · cross-host VM portability"
              stats={[]}
              compact
            />
            <BackendCard
              name="iscsi"
              badge="v1.6"
              badgeColor="text-caiman-amber bg-amber-900/20 border-caiman-amber/40"
              desc="iSCSI LUNs via LIO/tgtd"
              details="Per-VM block targets · multipath · BYO SAN"
              stats={[]}
              compact
            />
            <BackendCard
              name="caiman-storage"
              badge="v2.0"
              badgeColor="text-caiman-blue bg-blue-900/20 border-caiman-blue/40"
              desc="Hyperconverged distributed storage"
              details="vSAN-style · 2+ replicas across nodes · self-healing · requires federation"
              stats={[]}
              compact
              highlight
            />
          </div>
        </div>

        {/* Disabled "Create disk" hint */}
        <div className="bg-caiman-bg2 border border-caiman-border rounded p-3 flex items-center gap-3">
          <Lock size={12} className="text-caiman-dim flex-shrink-0" />
          <div className="flex-1 text-[10px] text-caiman-dim">
            Manual disk creation will be enabled when <span className="text-caiman-text">local-lvm</span> backend ships in v1.5. For now, disks are created automatically when you create a VM (CoW clone from base image).
          </div>
        </div>
      </div>
    </div>
  )
}

function BackendCard({ name, badge, badgeColor, desc, details, stats, compact, highlight }: any) {
  return (
    <div className={clsx(
      "bg-caiman-bg2 border rounded-lg overflow-hidden",
      highlight ? "border-caiman-blue/40 shadow-[0_0_20px_rgba(66,165,245,0.1)]" : "border-caiman-border"
    )}>
      <div className="px-4 py-3 flex items-start gap-3">
        <div className="flex-1">
          <div className="flex items-center gap-2 mb-1">
            <span className="text-[12px] text-[#e8f5e9] font-mono">{name}</span>
            <span className={clsx("px-1.5 py-0.5 text-[8px] tracking-[1.5px] uppercase rounded border", badgeColor)}>
              {badge}
            </span>
          </div>
          <div className="text-[10px] text-caiman-text mb-0.5">{desc}</div>
          <div className="text-[9px] text-caiman-dim">{details}</div>
        </div>
      </div>
      {!compact && stats.length > 0 && (
        <div className="px-4 py-2 border-t border-caiman-border bg-caiman-bg3 grid grid-cols-3 gap-4">
          {stats.map((s: any) => (
            <div key={s.label}>
              <div className="text-[8px] text-caiman-dim tracking-[1.5px] uppercase mb-0.5">{s.label}</div>
              <div className="text-[10px] text-caiman-text font-mono truncate">{s.value}</div>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}
