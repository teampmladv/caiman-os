import React, { useEffect, useState } from 'react'
import { Bell, Terminal, Settings, Zap } from 'lucide-react'
import { useClusterStore } from '../../store/cluster'
import { UserMenu } from './UserMenu'

const NAV_TABS = [
  { id: 'overview',  label: 'Overview' },
  { id: 'topology',  label: 'Topology' },
  { id: 'vms',       label: 'VMs' },
  { id: 'drs',       label: 'DRS' },
  { id: 'microseg',  label: 'Micro-seg' },
  { id: 'storage',   label: 'Storage' },
  { id: 'gpu',       label: 'GPU' },
]

interface Props {
  activeTab: string
  onTabChange: (tab: string) => void
}

export function TopNav({ activeTab, onTabChange }: Props) {
  const [time, setTime] = useState('')
  const { snapshot, unreadCount, openCommandBar, markAllRead } = useClusterStore(s => ({
    snapshot:       s.snapshot,
    unreadCount:    s.unreadCount,
    openCommandBar: s.openCommandBar,
    markAllRead:    s.markAllRead,
  }))

  useEffect(() => {
    const tick = () => setTime(new Date().toLocaleTimeString('en-GB', { hour12: false }))
    tick()
    const id = setInterval(tick, 1000)
    return () => clearInterval(id)
  }, [])

  // Global ⌘K shortcut
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        e.preventDefault()
        openCommandBar()
      }
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [openCommandBar])

  const vmCount     = snapshot?.vms.length ?? 0
  const runningVms  = snapshot?.vms.filter(v => v.status === 'RUNNING').length ?? 0
  const nodeCount   = snapshot?.nodes.length ?? 0
  const sigma       = snapshot?.balanceSigma.toFixed(3) ?? '—'
  const imbalanced  = (snapshot?.balanceSigma ?? 0) > 0.10

  return (
    <header className="h-11 bg-caiman-bg2 border-b border-caiman-border flex items-center px-3.5 gap-3 flex-shrink-0 z-20">
      {/* Logo */}
      <div className="flex items-center gap-2 mr-2 select-none">
        <div className="w-5 h-5 rounded-full border-[1.5px] border-caiman-green flex items-center justify-center">
          <div className="w-2.5 h-2.5 rounded-full bg-caiman-bright animate-heartbeat" />
        </div>
        <div>
          <div className="font-display text-[14px] font-semibold text-[#e8f5e9] leading-none tracking-wide">
            Caimán
          </div>
          <div className="text-[8px] text-caiman-dim tracking-[2px] leading-none mt-0.5">
            HAVANA · v0.1.0
          </div>
        </div>
      </div>

      {/* ⌘K Command Bar trigger */}
      <button
        onClick={openCommandBar}
        className="flex items-center gap-2 h-7 bg-caiman-bg3 border border-caiman-border
                   rounded-md px-2.5 text-caiman-dim hover:border-caiman-green
                   hover:text-caiman-green transition-colors duration-150 flex-shrink-0"
        style={{ width: 320 }}
      >
        <span className="text-[11px]">⌘</span>
        <span className="text-[10px] tracking-wide flex-1 text-left">
          Ask Claude anything about the cluster…
        </span>
        <kbd className="text-[8px] px-1.5 py-0.5 border border-caiman-border rounded bg-caiman-bg">
          ⌘K
        </kbd>
      </button>

      {/* Tab navigation */}
      <nav className="flex gap-0.5 ml-2 overflow-x-auto">
        {NAV_TABS.map(tab => (
          <button
            key={tab.id}
            onClick={() => onTabChange(tab.id)}
            className={[
              'px-3 py-1 text-[9px] tracking-[2px] uppercase font-mono whitespace-nowrap',
              'border-b-2 transition-all duration-150',
              activeTab === tab.id
                ? 'text-caiman-bright border-caiman-bright'
                : 'text-caiman-dim border-transparent hover:text-caiman-text',
            ].join(' ')}
          >
            {tab.label}
          </button>
        ))}
      </nav>

      {/* Right side */}
      <div className="ml-auto flex items-center gap-2.5">
        {/* Cluster status */}
        <div className="hidden sm:flex items-center gap-4 text-[9px] text-caiman-dim">
          <span>
            <span className="text-caiman-text">{nodeCount}</span> nodes
          </span>
          <span>
            <span className="text-caiman-bright">{runningVms}</span>/{vmCount} VMs
          </span>
          <span className={imbalanced ? 'text-caiman-amber' : ''}>
            σ={sigma}
          </span>
        </div>

        {/* Live badge */}
        <div className="flex items-center gap-1.5 text-[9px] text-caiman-bright px-2 py-0.5
                        rounded bg-[#0d2e0d] border border-caiman-border2">
          <div className="live-dot" />
          LIVE
        </div>

        {/* Alerts bell */}
        <button
          onClick={markAllRead}
          className="relative btn-ghost p-1.5 rounded"
        >
          <Bell size={14} className={unreadCount > 0 ? 'text-caiman-amber' : 'text-caiman-dim'} />
          {unreadCount > 0 && (
            <span className="absolute -top-1 -right-1 w-3.5 h-3.5 rounded-full bg-caiman-amber
                             text-[#000] text-[7px] flex items-center justify-center font-bold">
              {unreadCount > 9 ? '9+' : unreadCount}
            </span>
          )}
        </button>

        <UserMenu />
        <div className="font-mono text-[9px] text-caiman-dim w-14 text-right tabular-nums">
          {time}
        </div>
      </div>
    </header>
  )
}
