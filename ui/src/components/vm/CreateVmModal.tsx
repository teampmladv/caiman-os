import React, { useState, useRef, useEffect } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import { X, Cpu, MemoryStick, Network, Loader2, Plus } from 'lucide-react'
import { useCreateVm } from '../../framework/mutations'

interface Props {
  onClose: () => void
}

const PRESETS = [
  { label: 'Nano',   cpus: 1, mem: 256,  desc: 'Quick testing'    },
  { label: 'Micro',  cpus: 1, mem: 512,  desc: 'Lightweight apps' },
  { label: 'Small',  cpus: 2, mem: 1024, desc: 'Web servers'      },
  { label: 'Medium', cpus: 4, mem: 2048, desc: 'Databases'        },
]

export function CreateVmModal({ onClose }: Props) {
  const [name,     setName]     = useState('')
  const [cpus,     setCpus]     = useState(1)
  const [memMib,   setMemMib]   = useState(256)
  const [netMode,  setNetMode]  = useState<'nat' | 'bridge' | 'none'>('nat')
  const [preset,   setPreset]   = useState<number | null>(0)
  const nameRef = useRef<HTMLInputElement>(null)
  const create  = useCreateVm()

  useEffect(() => { nameRef.current?.focus() }, [])

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose()
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [onClose])

  const applyPreset = (i: number) => {
    setPreset(i)
    setCpus(PRESETS[i].cpus)
    setMemMib(PRESETS[i].mem)
  }

  const handleSubmit = async (e?: React.FormEvent) => {
    e?.preventDefault()
    if (!name || create.isPending) return
    try {
      await create.mutateAsync({ name, cpus, memMib, netMode } as any)
      onClose()
    } catch {}
  }

  const valid = name.trim().length >= 2 && cpus >= 1 && memMib >= 64

  return (
    <AnimatePresence>
      <motion.div
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0 }}
        className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm"
        onClick={e => { if (e.target === e.currentTarget) onClose() }}
      >
        <motion.div
          initial={{ scale: 0.95, opacity: 0 }}
          animate={{ scale: 1,    opacity: 1 }}
          exit={{    scale: 0.95, opacity: 0 }}
          transition={{ type: 'spring', stiffness: 400, damping: 35 }}
          className="bg-caiman-bg2 border border-caiman-border rounded-lg shadow-[0_20px_60px_rgba(0,0,0,0.5)] w-[520px] max-w-[95vw] max-h-[90vh] overflow-hidden flex flex-col"
        >
          <div className="flex items-center gap-2 px-4 py-3 border-b border-caiman-border bg-caiman-bg3">
            <Plus size={13} className="text-caiman-green" />
            <span className="text-[11px] text-[#e8f5e9] font-mono flex-1 tracking-wide">New virtual machine</span>
            <button onClick={onClose} className="text-caiman-dim hover:text-caiman-bright p-0.5">
              <X size={13} />
            </button>
          </div>

          <form onSubmit={handleSubmit} className="px-5 py-4 flex flex-col gap-4 overflow-y-auto">
            <div>
              <label className="text-[8px] text-caiman-dim tracking-[2px] uppercase block mb-1.5">Name</label>
              <input
                ref={nameRef}
                type="text"
                value={name}
                onChange={e => setName(e.target.value)}
                placeholder="web-server-01"
                spellCheck={false}
                className="w-full bg-caiman-bg border border-caiman-border rounded px-3 py-2 text-[12px] text-[#e8f5e9] font-mono focus:border-caiman-green focus:outline-none transition-colors"
              />
            </div>

            <div>
              <label className="text-[8px] text-caiman-dim tracking-[2px] uppercase block mb-1.5">Preset</label>
              <div className="grid grid-cols-4 gap-1.5">
                {PRESETS.map((p, i) => (
                  <button
                    type="button"
                    key={p.label}
                    onClick={() => applyPreset(i)}
                    className={"px-2 py-2 rounded border text-left transition-all " + (preset === i ? "border-caiman-green bg-caiman-green/10 text-caiman-bright" : "border-caiman-border bg-caiman-bg text-caiman-text hover:border-caiman-border2")}
                  >
                    <div className="text-[10px] font-mono">{p.label}</div>
                    <div className="text-[8px] text-caiman-dim mt-0.5">{p.cpus}c {p.mem}M</div>
                  </button>
                ))}
              </div>
            </div>

            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="text-[8px] text-caiman-dim tracking-[2px] uppercase block mb-1.5 flex items-center gap-1.5">
                  <Cpu size={9} /> vCPU
                </label>
                <input
                  type="number"
                  min={1} max={64}
                  value={cpus}
                  onChange={e => { setPreset(null); setCpus(Math.max(1, parseInt(e.target.value) || 1)) }}
                  className="w-full bg-caiman-bg border border-caiman-border rounded px-3 py-2 text-[12px] text-[#e8f5e9] font-mono focus:border-caiman-green focus:outline-none"
                />
              </div>
              <div>
                <label className="text-[8px] text-caiman-dim tracking-[2px] uppercase block mb-1.5 flex items-center gap-1.5">
                  <MemoryStick size={9} /> Memory (MiB)
                </label>
                <input
                  type="number"
                  min={64} step={64}
                  value={memMib}
                  onChange={e => { setPreset(null); setMemMib(Math.max(64, parseInt(e.target.value) || 64)) }}
                  className="w-full bg-caiman-bg border border-caiman-border rounded px-3 py-2 text-[12px] text-[#e8f5e9] font-mono focus:border-caiman-green focus:outline-none"
                />
              </div>
            </div>

            <div>
              <label className="text-[8px] text-caiman-dim tracking-[2px] uppercase block mb-1.5 flex items-center gap-1.5">
                <Network size={9} /> Network mode
              </label>
              <div className="grid grid-cols-3 gap-1.5">
                {(['nat', 'bridge', 'none'] as const).map(mode => (
                  <button
                    type="button"
                    key={mode}
                    onClick={() => setNetMode(mode)}
                    className={"px-2 py-1.5 rounded border text-[10px] font-mono uppercase tracking-wide transition-all " + (netMode === mode ? "border-caiman-green bg-caiman-green/10 text-caiman-bright" : "border-caiman-border bg-caiman-bg text-caiman-text hover:border-caiman-border2")}
                  >
                    {mode}
                  </button>
                ))}
              </div>
            </div>
          </form>

          <div className="flex items-center gap-2 px-5 py-3 border-t border-caiman-border bg-caiman-bg3">
            <span className="text-[8px] text-caiman-dim tracking-[1.5px]">ESC to cancel</span>
            <div className="flex-1" />
            <button
              type="button"
              onClick={onClose}
              className="px-3 py-1.5 text-[10px] tracking-[1.5px] uppercase text-caiman-dim hover:text-caiman-text"
            >Cancel</button>
            <button
              type="button"
              onClick={() => handleSubmit()}
              disabled={!valid || create.isPending}
              className="px-4 py-1.5 rounded bg-caiman-green/15 border border-caiman-green/50 text-caiman-bright text-[10px] tracking-[2px] uppercase hover:bg-caiman-green/25 hover:border-caiman-green disabled:opacity-30 disabled:cursor-not-allowed transition-all flex items-center gap-2"
            >
              {create.isPending ? <><Loader2 size={11} className="animate-spin" /> Creating...</> : <>Create VM</>}
            </button>
          </div>
        </motion.div>
      </motion.div>
    </AnimatePresence>
  )
}
