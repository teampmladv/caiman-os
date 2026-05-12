import React, { useEffect, useRef, useState } from 'react'
import { AnimatePresence, motion } from 'framer-motion'
import { Command } from 'cmdk'
import { useClusterStore } from '../../store/cluster'
import { ArrowRight, Zap, Server, Network, ShieldCheck, HardDrive, RefreshCw, Cpu } from 'lucide-react'

const SUGGESTIONS = [
  { icon: RefreshCw,    cat: 'DRS',      text: 'Suggest a migration for the most loaded VM' },
  { icon: ShieldCheck,  cat: 'Microseg', text: 'Show XDP deny events in the last 60 seconds' },
  { icon: Cpu,          cat: 'GPU',      text: 'Show GPU availability across the cluster' },
  { icon: HardDrive,    cat: 'Storage',  text: 'Create 500 GiB VSAN volume with FTT=1' },
  { icon: Server,       cat: 'VMs',      text: 'Why is the cluster under load? Suggest a fix' },
  { icon: Network,      cat: 'Network',  text: 'Show XDP throughput breakdown by VM' },
  { icon: RefreshCw,    cat: 'DRS',      text: 'Execute all pending DRS migration recommendations' },
  { icon: ShieldCheck,  cat: 'Microseg', text: 'Create policy: allow backend → postgres on :5432' },
  { icon: Server,       cat: 'VMs',      text: 'Start vm-dev-build-01 on the least loaded node' },
  { icon: Zap,          cat: 'System',   text: 'What is the current cluster balance score?' },
]

const CAT_COLORS: Record<string, string> = {
  DRS:      'bg-[#0a1f2e] text-caiman-blue border-[#1565c0]',
  Microseg: 'bg-[#0d2e0d] text-caiman-bright border-caiman-border2',
  GPU:      'bg-[#1a1200] text-caiman-amber border-[#f57f17]',
  Storage:  'bg-[#1a0d1a] text-[#ce93d8] border-[#6a1b9a]',
  VMs:      'bg-caiman-bg3 text-caiman-text border-caiman-border',
  Network:  'bg-[#0a1525] text-[#64b5f6] border-[#0d47a1]',
  System:   'bg-caiman-bg3 text-caiman-muted border-caiman-border',
}

interface Props {
  onQuery: (q: string) => void
}

export function CommandBar({ onQuery }: Props) {
  const { commandBarOpen, closeCommandBar } = useClusterStore(s => ({
    commandBarOpen:  s.commandBarOpen,
    closeCommandBar: s.closeCommandBar,
  }))
  const [value, setValue] = useState('')
  const inputRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    if (commandBarOpen) {
      setValue('')
      setTimeout(() => inputRef.current?.focus(), 50)
    }
  }, [commandBarOpen])

  const filtered = value.trim()
    ? SUGGESTIONS.filter(s => s.text.toLowerCase().includes(value.toLowerCase()))
    : SUGGESTIONS

  const submit = (text: string) => {
    if (!text.trim()) return
    closeCommandBar()
    onQuery(text)
  }

  return (
    <AnimatePresence>
      {commandBarOpen && (
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.12 }}
          className="fixed inset-0 bg-black/60 flex items-start justify-center pt-20 z-50"
          onClick={closeCommandBar}
        >
          <motion.div
            initial={{ opacity: 0, scale: 0.96, y: -8 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.96, y: -8 }}
            transition={{ duration: 0.15 }}
            className="w-[540px] bg-caiman-bg3 border border-caiman-border2 rounded-xl
                       overflow-hidden shadow-panel"
            onClick={e => e.stopPropagation()}
          >
            {/* Input row */}
            <div className="flex items-center gap-2.5 px-3.5 py-2.5
                            border-b border-caiman-border">
              {/* Caiman eye */}
              <div className="w-5 h-5 rounded-full border-[1.5px] border-caiman-green
                              flex items-center justify-center flex-shrink-0">
                <div className="w-2.5 h-2.5 rounded-full bg-caiman-bright animate-heartbeat" />
              </div>
              <input
                ref={inputRef}
                value={value}
                onChange={e => setValue(e.target.value)}
                onKeyDown={e => {
                  if (e.key === 'Escape') closeCommandBar()
                  if (e.key === 'Enter') submit(value)
                }}
                placeholder="Ask Claude: migrate vm-prod, show XDP stats, create policy…"
                className="flex-1 bg-transparent border-none outline-none text-[12px]
                           text-[#e8f5e9] font-mono caret-caiman-bright
                           placeholder:text-caiman-dim"
              />
              <kbd className="text-[8px] px-1.5 py-0.5 border border-caiman-border
                              rounded bg-caiman-bg text-caiman-dim cursor-pointer"
                onClick={closeCommandBar}>
                ESC
              </kbd>
            </div>

            {/* Suggestions */}
            <div className="py-1.5 max-h-72 overflow-y-auto">
              <div className="text-[8px] text-caiman-dim tracking-[2px] uppercase
                              px-3.5 py-1.5 mb-0.5">
                {value ? `Results for "${value}"` : 'Quick actions'}
              </div>
              {filtered.length === 0 && (
                <div className="px-3.5 py-3 text-caiman-dim text-[11px]">
                  No suggestions — press Enter to ask Claude directly
                </div>
              )}
              {filtered.map((s, i) => {
                const Icon = s.icon
                return (
                  <button
                    key={i}
                    onClick={() => submit(s.text)}
                    className="w-full flex items-center gap-2.5 px-3 py-2
                               hover:bg-caiman-bg4 transition-colors duration-100
                               text-left group"
                  >
                    <div className="w-6 h-6 rounded-md bg-caiman-bg border border-caiman-border
                                    flex items-center justify-center flex-shrink-0 text-caiman-dim
                                    group-hover:text-caiman-green transition-colors">
                      <Icon size={11} />
                    </div>
                    <span className="flex-1 text-[11px] text-caiman-text group-hover:text-[#e8f5e9]
                                     transition-colors">
                      {s.text}
                    </span>
                    <span className={`text-[8px] px-1.5 py-0.5 rounded border tracking-wide
                                      ${CAT_COLORS[s.cat] ?? CAT_COLORS.System}`}>
                      {s.cat}
                    </span>
                    <ArrowRight size={10} className="text-caiman-dim opacity-0 group-hover:opacity-100
                                                     transition-opacity ml-1" />
                  </button>
                )
              })}
            </div>

            {/* Footer */}
            <div className="flex items-center justify-between px-3.5 py-2
                            border-t border-caiman-border text-[8px] text-caiman-dim">
              <span>↵ execute · ↑↓ navigate · ESC close</span>
              <span className="flex items-center gap-1.5 text-caiman-green">
                <div className="live-dot w-1 h-1" />
                Claude (caiman-mcp) connected
              </span>
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  )
}
