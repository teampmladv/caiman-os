import React, { useState, useMemo } from 'react'
import { Search, Play, StopCircle, Terminal, Trash2, Plus, Download, RefreshCw } from 'lucide-react'
import { useClusterStore } from '../store/cluster'
import { startVm, stopVm, deleteVm } from '../api/client'
import { CreateVmModal } from '../components/vm/CreateVmModal'
import { ImportModal } from '../components/vm/ImportModal'
import { ConsoleModal } from '../components/vm/ConsoleModal'
import toast from 'react-hot-toast'
import { clsx } from 'clsx'
import type { Vm } from '../types'

type SortKey = 'name' | 'status' | 'cpu' | 'mem' | 'uptime'

const STATUS_STYLES: Record<string, string> = {
  RUNNING:   'text-caiman-bright bg-caiman-green/10 border-caiman-green/40',
  STOPPED:   'text-caiman-dim bg-caiman-bg3 border-caiman-border',
  BOOTING:   'text-caiman-amber bg-amber-900/20 border-caiman-amber/40',
  MIGRATING: 'text-caiman-blue bg-blue-900/20 border-caiman-blue/40',
  ERROR:     'text-caiman-red bg-red-900/20 border-caiman-red/40',
}

export default function VMsPage() {
  const vms = useClusterStore(s => s.snapshot?.vms ?? [])
  const selectVm = useClusterStore(s => s.selectVm)

  const [query,   setQuery]   = useState('')
  const [sortBy,  setSortBy]  = useState<SortKey>('name')
  const [sortDir, setSortDir] = useState<'asc' | 'desc'>('asc')
  const [filter,  setFilter]  = useState<'all' | 'running' | 'stopped'>('all')
  const [createOpen, setCreateOpen] = useState(false)
  const [importOpen, setImportOpen] = useState(false)
  const [consoleVm,  setConsoleVm]  = useState<Vm | null>(null)
  const [actioning,  setActioning]  = useState<string | null>(null)

  const filtered = useMemo(() => {
    let r = vms
    if (filter === 'running') r = r.filter(v => v.status === 'RUNNING')
    if (filter === 'stopped') r = r.filter(v => v.status === 'STOPPED')
    if (query.trim()) {
      const q = query.toLowerCase()
      r = r.filter(v => v.name.toLowerCase().includes(q) || v.id.toLowerCase().includes(q))
    }
    return [...r].sort((a, b) => {
      let av: any, bv: any
      switch (sortBy) {
        case 'name':   av = a.name;        bv = b.name; break
        case 'status': av = a.status;      bv = b.status; break
        case 'cpu':    av = a.cpuUsagePct; bv = b.cpuUsagePct; break
        case 'mem':    av = a.memTotalMib; bv = b.memTotalMib; break
        case 'uptime': av = a.uptimeSecs;  bv = b.uptimeSecs; break
      }
      if (av < bv) return sortDir === 'asc' ? -1 : 1
      if (av > bv) return sortDir === 'asc' ?  1 : -1
      return 0
    })
  }, [vms, query, filter, sortBy, sortDir])

  const toggleSort = (k: SortKey) => {
    if (sortBy === k) setSortDir(d => d === 'asc' ? 'desc' : 'asc')
    else { setSortBy(k); setSortDir('asc') }
  }

  const handleStart = async (vm: Vm) => {
    setActioning(vm.id)
    try { await startVm(vm.id); toast.success(`${vm.name} starting...`) }
    catch (e: any) { toast.error(`Start failed: ${e.message}`) }
    finally { setActioning(null) }
  }
  const handleStop = async (vm: Vm) => {
    setActioning(vm.id)
    try { await stopVm(vm.id); toast.success(`${vm.name} stopped`) }
    catch (e: any) { toast.error(`Stop failed: ${e.message}`) }
    finally { setActioning(null) }
  }
  const handleDelete = async (vm: Vm) => {
    if (!confirm(`Delete ${vm.name}? This cannot be undone.`)) return
    setActioning(vm.id)
    try { await deleteVm(vm.id); toast.success(`${vm.name} deleted`) }
    catch (e: any) { toast.error(`Delete failed: ${e.message}`) }
    finally { setActioning(null) }
  }

  const fmtUptime = (s: number) => {
    if (!s) return '—'
    const d = Math.floor(s / 86400)
    const h = Math.floor((s % 86400) / 3600)
    const m = Math.floor((s % 3600) / 60)
    if (d) return `${d}d ${h}h`
    if (h) return `${h}h ${m}m`
    return `${m}m`
  }

  return (
    <div className="flex-1 flex flex-col overflow-hidden bg-caiman-bg">
      {/* Toolbar */}
      <div className="flex items-center gap-3 px-4 py-3 border-b border-caiman-border bg-caiman-bg2">
        <div className="text-[11px] text-[#e8f5e9] tracking-[2px] uppercase font-mono">Virtual machines</div>
        <span className="text-[9px] text-caiman-dim">
          {filtered.length} / {vms.length}
        </span>

        <div className="ml-4 flex items-center gap-1">
          {(['all', 'running', 'stopped'] as const).map(f => (
            <button
              key={f}
              onClick={() => setFilter(f)}
              className={clsx(
                "px-2 py-1 text-[9px] tracking-[1.5px] uppercase rounded",
                filter === f ? "bg-caiman-green/10 text-caiman-bright border border-caiman-green/40" : "text-caiman-dim hover:text-caiman-text"
              )}
            >{f}</button>
          ))}
        </div>

        <div className="flex-1" />

        <div className="relative">
          <Search size={11} className="absolute left-2 top-1/2 -translate-y-1/2 text-caiman-dim" />
          <input
            value={query}
            onChange={e => setQuery(e.target.value)}
            placeholder="Search..."
            className="bg-caiman-bg border border-caiman-border rounded pl-7 pr-2 py-1 text-[10px] text-[#e8f5e9] font-mono w-44 focus:border-caiman-green focus:outline-none"
          />
        </div>

        <button
          onClick={() => setImportOpen(true)}
          className="flex items-center gap-1.5 px-2.5 py-1 text-[9px] tracking-[1.5px] uppercase rounded border border-caiman-border text-caiman-text hover:border-caiman-green hover:text-caiman-bright transition-colors"
        >
          <Download size={10} /> Import
        </button>
        <button
          onClick={() => setCreateOpen(true)}
          className="flex items-center gap-1.5 px-2.5 py-1 text-[9px] tracking-[1.5px] uppercase rounded bg-caiman-green/15 border border-caiman-green/50 text-caiman-bright hover:bg-caiman-green/25 transition-colors"
        >
          <Plus size={10} /> New VM
        </button>
      </div>

      {/* Table */}
      <div className="flex-1 overflow-auto">
        <table className="w-full text-[10px] font-mono">
          <thead className="sticky top-0 bg-caiman-bg2 border-b border-caiman-border">
            <tr className="text-[8px] text-caiman-dim tracking-[1.5px] uppercase">
              <ThCell label="Name"    onClick={() => toggleSort('name')}    active={sortBy === 'name'}    dir={sortDir} />
              <ThCell label="Status"  onClick={() => toggleSort('status')}  active={sortBy === 'status'}  dir={sortDir} />
              <ThCell label="vCPU"    onClick={() => toggleSort('cpu')}     active={sortBy === 'cpu'}     dir={sortDir} align="right" />
              <ThCell label="Memory"  onClick={() => toggleSort('mem')}     active={sortBy === 'mem'}     dir={sortDir} align="right" />
              <th className="px-3 py-2 text-left">IP</th>
              <th className="px-3 py-2 text-left">Node</th>
              <ThCell label="Uptime"  onClick={() => toggleSort('uptime')}  active={sortBy === 'uptime'}  dir={sortDir} />
              <th className="px-3 py-2 text-right">Actions</th>
            </tr>
          </thead>
          <tbody>
            {filtered.map(vm => {
              const status = STATUS_STYLES[vm.status] ?? STATUS_STYLES.STOPPED
              const isActioning = actioning === vm.id
              return (
                <tr
                  key={vm.id}
                  onClick={() => selectVm(vm.id)}
                  className="border-b border-caiman-border hover:bg-caiman-bg2 cursor-pointer transition-colors"
                >
                  <td className="px-3 py-2 text-[#e8f5e9]">
                    <div>{vm.name}</div>
                    <div className="text-[8px] text-caiman-dim">{vm.id}</div>
                  </td>
                  <td className="px-3 py-2">
                    <span className={clsx("inline-flex items-center gap-1 px-1.5 py-0.5 text-[8px] tracking-[1.5px] uppercase rounded border", status)}>
                      {vm.status !== 'STOPPED' && vm.status !== 'ERROR' &&
                        <span className="w-1 h-1 rounded-full bg-current animate-pulse" />}
                      {vm.status}
                    </span>
                  </td>
                  <td className="px-3 py-2 text-right text-caiman-text">{vm.cpuCores}</td>
                  <td className="px-3 py-2 text-right text-caiman-text">{Math.round(vm.memTotalMib / 1024 * 10) / 10}G</td>
                  <td className="px-3 py-2 text-caiman-text">{(vm as any).ip || '—'}</td>
                  <td className="px-3 py-2 text-caiman-dim">{vm.nodeName}</td>
                  <td className="px-3 py-2 text-caiman-dim">{fmtUptime(vm.uptimeSecs)}</td>
                  <td className="px-3 py-2" onClick={e => e.stopPropagation()}>
                    <div className="flex items-center justify-end gap-1">
                      {vm.status === 'RUNNING' && (
                        <ActionBtn title="Console" onClick={() => setConsoleVm(vm)}><Terminal size={11} /></ActionBtn>
                      )}
                      {vm.status === 'RUNNING' ? (
                        <ActionBtn title="Stop" danger disabled={isActioning} onClick={() => handleStop(vm)}><StopCircle size={11} /></ActionBtn>
                      ) : (
                        <ActionBtn title="Start" disabled={isActioning} onClick={() => handleStart(vm)}>
                          {isActioning ? <RefreshCw size={11} className="animate-spin" /> : <Play size={11} />}
                        </ActionBtn>
                      )}
                      <ActionBtn title="Delete" danger disabled={isActioning} onClick={() => handleDelete(vm)}><Trash2 size={11} /></ActionBtn>
                    </div>
                  </td>
                </tr>
              )
            })}
            {filtered.length === 0 && (
              <tr><td colSpan={8} className="text-center py-8 text-caiman-dim text-[10px]">No VMs match the current filter</td></tr>
            )}
          </tbody>
        </table>
      </div>

      {createOpen  && <CreateVmModal onClose={() => setCreateOpen(false)} />}
      {importOpen  && <ImportModal   onClose={() => setImportOpen(false)} />}
      {consoleVm   && <ConsoleModal vmId={consoleVm.id} vmName={consoleVm.name} onClose={() => setConsoleVm(null)} />}
    </div>
  )
}

function ThCell({ label, onClick, active, dir, align = 'left' }: any) {
  return (
    <th onClick={onClick} className={"px-3 py-2 cursor-pointer hover:text-caiman-text select-none " + (align === 'right' ? 'text-right' : 'text-left') + (active ? ' text-caiman-bright' : '')}>
      {label}{active && (dir === 'asc' ? ' ↑' : ' ↓')}
    </th>
  )
}

function ActionBtn({ children, onClick, title, danger, disabled }: {
  children: React.ReactNode; onClick: () => void; title: string; danger?: boolean; disabled?: boolean
}) {
  return (
    <button
      onClick={onClick}
      title={title}
      disabled={disabled}
      className={clsx(
        "p-1 rounded transition-colors disabled:opacity-30",
        danger
          ? "text-caiman-dim hover:text-caiman-red hover:bg-red-900/20"
          : "text-caiman-dim hover:text-caiman-bright hover:bg-caiman-bg3"
      )}
    >{children}</button>
  )
}
