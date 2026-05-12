import React, { useState, useEffect } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import {
  X, ArrowRight, ArrowLeft, Loader2, Check, AlertCircle,
  Download, Cloud, Server, Database, Box, Layers,
} from 'lucide-react'
import { api } from '../../api/client'
import toast from 'react-hot-toast'

interface Props {
  onClose: () => void
}

type Source = {
  id:    string
  name:  string
  desc:  string
  icon:  React.ElementType
  color: string
  fields: Array<{ key: string; label: string; placeholder: string; type?: string }>
}

const SOURCES: Source[] = [
  {
    id: 'proxmox', name: 'Proxmox VE',
    desc: 'Proxmox Virtual Environment cluster',
    icon: Server, color: '#e57005',
    fields: [
      { key: 'host', label: 'Host',     placeholder: 'pve.example.com' },
      { key: 'port', label: 'Port',     placeholder: '8006' },
      { key: 'user', label: 'User',     placeholder: 'root@pam' },
      { key: 'pass', label: 'Password', placeholder: '', type: 'password' },
    ],
  },
  {
    id: 'vsphere', name: 'VMware vSphere',
    desc: 'vCenter Server / ESXi host',
    icon: Layers, color: '#607078',
    fields: [
      { key: 'host', label: 'vCenter Host', placeholder: 'vcenter.example.com' },
      { key: 'user', label: 'Username',     placeholder: 'administrator@vsphere.local' },
      { key: 'pass', label: 'Password',     placeholder: '', type: 'password' },
    ],
  },
  {
    id: 'nutanix', name: 'Nutanix AHV',
    desc: 'Nutanix Prism Element / Central',
    icon: Box, color: '#024da1',
    fields: [
      { key: 'host', label: 'Prism Host', placeholder: 'prism.example.com' },
      { key: 'user', label: 'Username',   placeholder: 'admin' },
      { key: 'pass', label: 'Password',   placeholder: '', type: 'password' },
    ],
  },
  {
    id: 'olvm', name: 'Oracle Linux VM',
    desc: 'OLVM / oVirt-based cluster',
    icon: Database, color: '#c74634',
    fields: [
      { key: 'host', label: 'Engine Host', placeholder: 'olvm.example.com' },
      { key: 'user', label: 'Username',    placeholder: 'admin@internal' },
      { key: 'pass', label: 'Password',    placeholder: '', type: 'password' },
    ],
  },
  {
    id: 'oraclevm', name: 'Oracle VM Server',
    desc: 'Oracle VM Manager (Xen-based)',
    icon: Database, color: '#c74634',
    fields: [
      { key: 'host', label: 'OVM Manager', placeholder: 'ovmm.example.com' },
      { key: 'user', label: 'Username',    placeholder: 'admin' },
      { key: 'pass', label: 'Password',    placeholder: '', type: 'password' },
    ],
  },
  {
    id: 'openstack', name: 'OpenStack',
    desc: 'OpenStack Nova / Glance',
    icon: Cloud, color: '#ed1944',
    fields: [
      { key: 'host', label: 'Keystone URL', placeholder: 'https://os.example.com:5000' },
      { key: 'user', label: 'Username',     placeholder: 'admin' },
      { key: 'pass', label: 'Password',     placeholder: '', type: 'password' },
    ],
  },
  {
    id: 'harvester', name: 'Harvester HCI',
    desc: 'SUSE Harvester (KubeVirt)',
    icon: Layers, color: '#0c322c',
    fields: [
      { key: 'host', label: 'Harvester URL', placeholder: 'https://harvester.example.com' },
      { key: 'user', label: 'Username',      placeholder: 'admin' },
      { key: 'pass', label: 'Password',      placeholder: '', type: 'password' },
    ],
  },
  {
    id: 'libvirt', name: 'libvirt / KVM',
    desc: 'Generic libvirt host (qemu+ssh)',
    icon: Server, color: '#cc0000',
    fields: [
      { key: 'host', label: 'Host', placeholder: 'qemu+ssh://root@kvm-host/system' },
    ],
  },
  {
    id: 'aws', name: 'AWS EC2',
    desc: 'Amazon Elastic Compute Cloud',
    icon: Cloud, color: '#ff9900',
    fields: [
      { key: 'key',    label: 'Access Key ID',     placeholder: 'AKIA...' },
      { key: 'secret', label: 'Secret Access Key', placeholder: '', type: 'password' },
      { key: 'region', label: 'Region',            placeholder: 'eu-west-1' },
    ],
  },
]

interface SourceVm {
  id: string; source_id: string; name: string
  cpus: number; mem_mib: number; disk_gb: number
  os: string; status: string
}

export function ImportModal({ onClose }: Props) {
  const [step,        setStep]        = useState(1)
  const [source,      setSource]      = useState<Source | null>(null)
  const [creds,       setCreds]       = useState<Record<string, string>>({})
  const [discovering, setDiscovering] = useState(false)
  const [vms,         setVms]         = useState<SourceVm[]>([])
  const [selected,    setSelected]    = useState<Set<string>>(new Set())
  const [importing,   setImporting]   = useState(false)
  const [error,       setError]       = useState<string | null>(null)
  const [progress,    setProgress]    = useState<Record<string, string>>({})

  useEffect(() => {
    const h = (e: KeyboardEvent) => { if (e.key === 'Escape') onClose() }
    window.addEventListener('keydown', h)
    return () => window.removeEventListener('keydown', h)
  }, [onClose])

  const pickSource = (s: Source) => {
    setSource(s)
    setCreds({})
    setError(null)
    setStep(2)
  }

  const handleDiscover = async () => {
    if (!source) return
    setDiscovering(true)
    setError(null)
    try {
      const r = await api.post('/api/import/discover', { source: source.id, credentials: creds })
      const found: SourceVm[] = r.data.vms ?? []
      setVms(found)
      if (found.length === 0) {
        setError(`No VMs found on ${source.name}`)
      } else {
        setStep(3)
      }
    } catch (e: any) {
      setError(e?.response?.data?.error ?? e?.message ?? 'Discovery failed')
    } finally {
      setDiscovering(false)
    }
  }

  const toggleVm = (id: string) => {
    setSelected(s => {
      const n = new Set(s)
      if (n.has(id)) n.delete(id); else n.add(id)
      return n
    })
  }

  const handleImport = async () => {
    if (!source || selected.size === 0) return
    setImporting(true)
    const toImport = vms.filter(v => selected.has(v.id))
    for (const vm of toImport) {
      setProgress(p => ({ ...p, [vm.id]: 'importing' }))
      try {
        await api.post('/api/import/vm', { source: source.id, vm })
        setProgress(p => ({ ...p, [vm.id]: 'done' }))
      } catch (e: any) {
        setProgress(p => ({ ...p, [vm.id]: 'failed' }))
      }
    }
    setImporting(false)
    const ok = Object.values(progress).filter(s => s === 'done').length
    toast.success(`${ok}/${toImport.length} VMs imported`)
    setTimeout(onClose, 1500)
  }

  const credsValid = source?.fields.every(f => creds[f.key]?.trim()) ?? false

  return (
    <AnimatePresence>
      <motion.div
        initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }}
        className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm"
        onClick={e => { if (e.target === e.currentTarget && !importing) onClose() }}
      >
        <motion.div
          initial={{ scale: 0.95, opacity: 0 }}
          animate={{ scale: 1,    opacity: 1 }}
          exit={{    scale: 0.95, opacity: 0 }}
          transition={{ type: 'spring', stiffness: 400, damping: 35 }}
          className="bg-caiman-bg2 border border-caiman-border rounded-lg shadow-[0_20px_60px_rgba(0,0,0,0.5)] w-[680px] max-w-[95vw] max-h-[85vh] overflow-hidden flex flex-col"
        >
          {/* Header */}
          <div className="flex items-center gap-2 px-4 py-3 border-b border-caiman-border bg-caiman-bg3">
            <Download size={13} className="text-caiman-green" />
            <span className="text-[11px] text-[#e8f5e9] font-mono flex-1 tracking-wide">
              Import VMs {source && <span className="text-caiman-dim">/ {source.name}</span>}
            </span>
            <div className="flex items-center gap-1 text-[8px] text-caiman-dim tracking-[1.5px]">
              <span className={step >= 1 ? 'text-caiman-green' : ''}>SOURCE</span>
              <ArrowRight size={9} />
              <span className={step >= 2 ? 'text-caiman-green' : ''}>CONNECT</span>
              <ArrowRight size={9} />
              <span className={step >= 3 ? 'text-caiman-green' : ''}>IMPORT</span>
            </div>
            <button onClick={onClose} disabled={importing} className="text-caiman-dim hover:text-caiman-bright p-0.5 ml-2">
              <X size={13} />
            </button>
          </div>

          {/* Body */}
          <div className="flex-1 overflow-y-auto">
            {/* STEP 1: source picker */}
            {step === 1 && (
              <div className="p-5 grid grid-cols-3 gap-2">
                {SOURCES.map(s => {
                  const Icon = s.icon
                  return (
                    <button
                      key={s.id}
                      onClick={() => pickSource(s)}
                      className="text-left p-3 rounded border border-caiman-border bg-caiman-bg hover:border-caiman-green hover:bg-caiman-bg3 transition-all group"
                    >
                      <div className="flex items-center gap-2 mb-1.5">
                        <Icon size={14} style={{ color: s.color }} className="opacity-80 group-hover:opacity-100" />
                        <span className="text-[11px] text-[#e8f5e9] font-mono">{s.name}</span>
                      </div>
                      <div className="text-[9px] text-caiman-dim leading-snug">{s.desc}</div>
                    </button>
                  )
                })}
              </div>
            )}

            {/* STEP 2: credentials */}
            {step === 2 && source && (
              <div className="p-5 flex flex-col gap-3">
                <div className="flex items-center gap-2 mb-1">
                  <source.icon size={14} style={{ color: source.color }} />
                  <span className="text-[12px] text-[#e8f5e9] font-mono">{source.name}</span>
                </div>
                <div className="text-[10px] text-caiman-dim mb-2">{source.desc}</div>
                {source.fields.map(f => (
                  <div key={f.key}>
                    <label className="text-[8px] text-caiman-dim tracking-[2px] uppercase block mb-1.5">{f.label}</label>
                    <input
                      type={f.type ?? 'text'}
                      value={creds[f.key] ?? ''}
                      onChange={e => setCreds({ ...creds, [f.key]: e.target.value })}
                      placeholder={f.placeholder}
                      spellCheck={false}
                      autoComplete={f.type === 'password' ? 'new-password' : 'off'}
                      className="w-full bg-caiman-bg border border-caiman-border rounded px-3 py-2 text-[11px] text-[#e8f5e9] font-mono focus:border-caiman-green focus:outline-none"
                    />
                  </div>
                ))}
                {error && (
                  <div className="flex items-center gap-2 text-[10px] text-caiman-red bg-red-900/20 border border-caiman-red/40 rounded px-2.5 py-2 mt-1">
                    <AlertCircle size={11} /> {error}
                  </div>
                )}
              </div>
            )}

            {/* STEP 3: VM list */}
            {step === 3 && source && (
              <div className="flex flex-col">
                <div className="px-4 py-2 border-b border-caiman-border flex items-center justify-between bg-caiman-bg">
                  <span className="text-[10px] text-caiman-dim tracking-[1.5px] uppercase">
                    {vms.length} VMs found · {selected.size} selected
                  </span>
                  <button
                    onClick={() => setSelected(selected.size === vms.length ? new Set() : new Set(vms.map(v => v.id)))}
                    className="text-[9px] text-caiman-green hover:text-caiman-bright tracking-wide"
                  >
                    {selected.size === vms.length ? 'Deselect all' : 'Select all'}
                  </button>
                </div>
                <div className="overflow-y-auto max-h-[400px]">
                  {vms.map(vm => {
                    const isSelected = selected.has(vm.id)
                    const state = progress[vm.id]
                    return (
                      <button
                        key={vm.id}
                        onClick={() => !importing && toggleVm(vm.id)}
                        disabled={importing}
                        className={`w-full px-4 py-2 border-b border-caiman-border text-left flex items-center gap-3 hover:bg-caiman-bg3 transition-colors ${isSelected ? 'bg-caiman-green/5' : ''}`}
                      >
                        <div className={`w-3.5 h-3.5 rounded border flex-shrink-0 flex items-center justify-center ${isSelected ? 'border-caiman-green bg-caiman-green/20' : 'border-caiman-border'}`}>
                          {isSelected && <Check size={9} className="text-caiman-bright" />}
                        </div>
                        <div className="flex-1 min-w-0">
                          <div className="text-[11px] text-[#e8f5e9] font-mono truncate">{vm.name}</div>
                          <div className="text-[9px] text-caiman-dim">{vm.cpus} vCPU · {Math.round(vm.mem_mib/1024)}GB · {vm.disk_gb}GB · {vm.os}</div>
                        </div>
                        <span className={`text-[8px] tracking-[1.5px] uppercase px-1.5 py-0.5 rounded border ${vm.status === 'running' ? 'text-caiman-green border-caiman-border2' : 'text-caiman-dim border-caiman-border'}`}>
                          {vm.status}
                        </span>
                        {state === 'importing' && <Loader2 size={11} className="text-caiman-amber animate-spin" />}
                        {state === 'done'      && <Check size={11} className="text-caiman-green" />}
                        {state === 'failed'    && <X size={11} className="text-caiman-red" />}
                      </button>
                    )
                  })}
                </div>
              </div>
            )}
          </div>

          {/* Footer */}
          <div className="flex items-center gap-2 px-5 py-3 border-t border-caiman-border bg-caiman-bg3">
            {step > 1 && step < 3 && (
              <button
                onClick={() => setStep(s => s - 1)}
                className="px-3 py-1.5 text-[10px] tracking-[1.5px] uppercase text-caiman-dim hover:text-caiman-text flex items-center gap-1"
              >
                <ArrowLeft size={10} /> Back
              </button>
            )}
            <div className="flex-1" />
            <button
              onClick={onClose}
              disabled={importing}
              className="px-3 py-1.5 text-[10px] tracking-[1.5px] uppercase text-caiman-dim hover:text-caiman-text"
            >Cancel</button>

            {step === 2 && (
              <button
                onClick={handleDiscover}
                disabled={!credsValid || discovering}
                className="px-4 py-1.5 rounded bg-caiman-green/15 border border-caiman-green/50 text-caiman-bright text-[10px] tracking-[2px] uppercase hover:bg-caiman-green/25 hover:border-caiman-green disabled:opacity-30 disabled:cursor-not-allowed transition-all flex items-center gap-2"
              >
                {discovering ? <><Loader2 size={11} className="animate-spin" /> Discovering...</> : <>Discover <ArrowRight size={10} /></>}
              </button>
            )}

            {step === 3 && (
              <button
                onClick={handleImport}
                disabled={selected.size === 0 || importing}
                className="px-4 py-1.5 rounded bg-caiman-green/15 border border-caiman-green/50 text-caiman-bright text-[10px] tracking-[2px] uppercase hover:bg-caiman-green/25 hover:border-caiman-green disabled:opacity-30 disabled:cursor-not-allowed transition-all flex items-center gap-2"
              >
                {importing ? <><Loader2 size={11} className="animate-spin" /> Importing...</> : <>Import {selected.size > 0 ? `(${selected.size})` : ''}</>}
              </button>
            )}
          </div>
        </motion.div>
      </motion.div>
    </AnimatePresence>
  )
}
