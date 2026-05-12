import React from 'react'
import { Cpu, Zap, Info, Sparkles, AlertCircle } from 'lucide-react'
import { clsx } from 'clsx'

export default function GPUPage() {
  // In real deployment, this would come from /api/gpu/devices
  const gpus: any[] = []
  const iommuEnabled = false // would check /sys/kernel/iommu_groups

  return (
    <div className="flex-1 overflow-auto bg-caiman-bg">
      <div className="px-5 py-3 border-b border-caiman-border bg-caiman-bg2 flex items-center gap-4">
        <Cpu size={13} className="text-caiman-green" />
        <div className="text-[11px] text-[#e8f5e9] tracking-[2px] uppercase font-mono">GPU passthrough</div>
        <span className="text-[9px] text-caiman-dim">
          {gpus.length} device{gpus.length !== 1 ? 's' : ''} detected
        </span>
      </div>

      <div className="p-6 max-w-[1100px] mx-auto space-y-6">
        {/* IOMMU status */}
        <div className={clsx(
          "rounded-lg border p-4 flex items-start gap-3",
          iommuEnabled ? "border-caiman-green/30 bg-caiman-green/5" : "border-caiman-amber/40 bg-amber-900/10"
        )}>
          {iommuEnabled
            ? <Zap size={14} className="text-caiman-bright mt-0.5 flex-shrink-0" />
            : <AlertCircle size={14} className="text-caiman-amber mt-0.5 flex-shrink-0" />
          }
          <div className="flex-1 text-[10px] leading-relaxed">
            <div className={`font-mono text-[11px] mb-1 ${iommuEnabled ? 'text-caiman-bright' : 'text-caiman-amber'}`}>
              IOMMU {iommuEnabled ? 'enabled' : 'not detected'}
            </div>
            <div className="text-caiman-dim">
              {iommuEnabled
                ? 'IOMMU groups available. GPU passthrough is supported.'
                : <>GPU passthrough requires IOMMU (VT-d / AMD-Vi) enabled in BIOS, plus <span className="font-mono text-caiman-text">intel_iommu=on iommu=pt</span> (Intel) or <span className="font-mono text-caiman-text">amd_iommu=on iommu=pt</span> (AMD) in the kernel cmdline.</>
              }
            </div>
          </div>
        </div>

        {/* GPU devices */}
        <div className="bg-caiman-bg2 border border-caiman-border rounded-lg overflow-hidden">
          <div className="px-4 py-2.5 border-b border-caiman-border flex items-center gap-2">
            <Cpu size={11} className="text-caiman-dim" />
            <span className="text-[10px] text-caiman-text tracking-[1.5px] uppercase">GPU devices</span>
          </div>
          {gpus.length === 0 ? (
            <div className="p-8 text-center">
              <Cpu size={28} className="text-caiman-dim mx-auto mb-3 opacity-40" />
              <div className="text-[11px] text-caiman-text mb-1">No GPUs detected</div>
              <div className="text-[9px] text-caiman-dim max-w-md mx-auto leading-relaxed">
                Caimán detects NVIDIA, AMD, and Intel GPUs at boot. If a GPU is installed but not listed,
                ensure it appears in <span className="font-mono text-caiman-text">lspci</span> and IOMMU is enabled.
              </div>
            </div>
          ) : (
            <div>{/* GPU list would render here */}</div>
          )}
        </div>

        {/* Modes */}
        <div>
          <div className="text-[9px] text-caiman-dim tracking-[2px] uppercase mb-2">Allocation modes</div>
          <div className="grid grid-cols-3 gap-3">
            <ModeCard
              name="Passthrough"
              version="partial"
              versionColor="amber"
              desc="Full GPU bound exclusively to one VM via VFIO-PCI. Native performance, no overhead."
              details="Bind unbind from host driver → vfio-pci → guest sees real PCIe device."
              status="VFIO flow implemented; tested on NVIDIA. Needs IOMMU."
            />
            <ModeCard
              name="MIG"
              version="v1.5"
              versionColor="dim"
              desc="NVIDIA Multi-Instance GPU. Split A100/H100 into up to 7 instances (1g.5gb, 2g.10gb, 3g.20gb, 7g.40gb)."
              details="Requires NVIDIA datacenter GPU + nvidia-smi MIG mode."
              status="Skeleton; nvidia-smi integration not implemented."
            />
            <ModeCard
              name="vGPU"
              version="v2.0"
              versionColor="dim"
              desc="NVIDIA vGPU virtualization. Many VMs share one physical GPU with time-slicing."
              details="Requires NVIDIA vGPU driver (licensed) and supported GPU (T4/A10/A40)."
              status="Skeleton; requires NVIDIA licensed driver."
            />
          </div>
        </div>

        {/* Coming */}
        <div>
          <div className="text-[9px] text-caiman-dim tracking-[2px] uppercase mb-2 flex items-center gap-2">
            <Sparkles size={10} /> Coming
          </div>
          <div className="bg-caiman-bg2 border border-caiman-border rounded p-3">
            <ul className="text-[10px] text-caiman-text space-y-1.5">
              <li>• <span className="text-caiman-bright">v1.5</span> — NVIDIA MIG integration (nvidia-smi mig)</li>
              <li>• <span className="text-caiman-bright">v1.6</span> — AMD GPU passthrough (ROCm devices)</li>
              <li>• <span className="text-caiman-bright">v1.7</span> — Intel SR-IOV (Flex/Arc datacenter)</li>
              <li>• <span className="text-caiman-bright">v2.0</span> — NVIDIA vGPU (licensed driver integration)</li>
              <li>• <span className="text-caiman-bright">v2.1</span> — Live migration of MIG instances</li>
            </ul>
          </div>
        </div>
      </div>
    </div>
  )
}

function ModeCard({ name, version, versionColor, desc, details, status }: any) {
  const vc = versionColor === 'amber'
    ? 'border-caiman-amber/40 text-caiman-amber bg-amber-900/20'
    : versionColor === 'dim'
    ? 'border-caiman-border text-caiman-dim bg-caiman-bg3'
    : 'border-caiman-green/40 text-caiman-bright bg-caiman-green/10'

  return (
    <div className="bg-caiman-bg2 border border-caiman-border rounded-lg p-3 flex flex-col">
      <div className="flex items-center gap-2 mb-1.5">
        <span className="text-[12px] text-[#e8f5e9] font-mono flex-1">{name}</span>
        <span className={clsx("px-1.5 py-0.5 text-[8px] tracking-[1.5px] uppercase rounded border", vc)}>
          {version}
        </span>
      </div>
      <div className="text-[10px] text-caiman-text mb-2 leading-relaxed">{desc}</div>
      <div className="text-[9px] text-caiman-dim mb-2 leading-relaxed">{details}</div>
      <div className="text-[9px] text-caiman-dim border-t border-caiman-border pt-2 mt-auto flex items-start gap-1.5">
        <Info size={9} className="mt-0.5 flex-shrink-0" />
        <span>{status}</span>
      </div>
    </div>
  )
}
