import React from 'react'
import {
  LayoutDashboard, Network, Server, Share2,
  ShieldCheck, HardDrive, Cpu, MessageSquare, Settings, RefreshCw,
} from 'lucide-react'
import { useClusterStore } from '../../store/cluster'
import { clsx } from 'clsx'

const ITEMS = [
  { id: 'overview',  icon: LayoutDashboard, tip: 'Overview' },
  { id: 'topology',  icon: Network,         tip: 'Topology' },
  { id: 'vms',       icon: Server,          tip: 'VMs' },
  { id: 'drs',       icon: RefreshCw,       tip: 'DRS' },
  null, // divider
  { id: 'microseg',  icon: ShieldCheck,     tip: 'Micro-seg' },
  { id: 'storage',   icon: HardDrive,       tip: 'Storage' },
  { id: 'gpu',       icon: Cpu,             tip: 'GPU' },
]

interface Props {
  activeTab: string
  onTabChange: (tab: string) => void
}

export function Sidebar({ activeTab, onTabChange }: Props) {
  const openCommandBar = useClusterStore(s => s.openCommandBar)

  return (
    <aside className="w-11 bg-caiman-bg2 border-r border-caiman-border flex flex-col
                      items-center py-2.5 gap-1 flex-shrink-0 z-10">
      {ITEMS.map((item, i) => {
        if (!item) {
          return <div key={`div-${i}`} className="w-6 h-px bg-caiman-border my-1" />
        }
        const Icon = item.icon
        const active = activeTab === item.id
        return (
          <button
            key={item.id}
            title={item.tip}
            onClick={() => onTabChange(item.id)}
            className={clsx(
              'w-[30px] h-[30px] rounded-md flex items-center justify-center',
              'border transition-all duration-150 group relative',
              active
                ? 'text-caiman-bright bg-caiman-bg3 border-caiman-border2'
                : 'text-caiman-dim border-transparent hover:text-caiman-text hover:bg-caiman-bg3',
            )}
          >
            <Icon size={13} />
            {/* Tooltip */}
            <span className="absolute left-full ml-2 px-2 py-1 rounded text-[9px]
                             bg-caiman-bg3 border border-caiman-border text-caiman-text
                             opacity-0 group-hover:opacity-100 whitespace-nowrap
                             pointer-events-none transition-opacity duration-150 z-50
                             tracking-wide">
              {item.tip}
            </span>
          </button>
        )
      })}

      {/* Spacer */}
      <div className="flex-1" />
      <div className="w-6 h-px bg-caiman-border mb-1" />

      {/* Claude MCP */}
      <button
        title="Claude AI (⌘K)"
        onClick={openCommandBar}
        className="w-[30px] h-[30px] rounded-md flex items-center justify-center
                   border border-caiman-border2 text-caiman-green bg-[#0d2e0d]
                   hover:text-caiman-bright hover:shadow-caiman-glow
                   transition-all duration-150"
      >
        <MessageSquare size={13} />
      </button>

      {/* Settings */}
      <button
        title="Settings"
        className="w-[30px] h-[30px] rounded-md flex items-center justify-center
                   border border-transparent text-caiman-dim
                   hover:text-caiman-text hover:bg-caiman-bg3
                   transition-all duration-150"
      >
        <Settings size={13} />
      </button>
    </aside>
  )
}
