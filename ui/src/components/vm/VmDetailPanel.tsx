import React, { useEffect, useRef } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import { X, Terminal, RefreshCw, StopCircle, Play, Cpu, MemoryStick, Network, HardDrive, Trash2, AlertTriangle } from 'lucide-react'
import { useClusterStore, selectSelectedVm } from '../../store/cluster'
import { clsx } from 'clsx'
import type { Vm } from '../../types'
import { startVm, stopVm, deleteVm } from '../../api/client'
import toast from 'react-hot-toast'
import { ConsoleModal } from './ConsoleModal'

function MiniStat({ icon: Icon, label, value, color = '' }: {
  icon: React.ElementType; label: string; value: string; color?: string
}) {
  return (
    <div className="flex flex-col bg-caiman-bg p-2.5 rounded-md border border-caiman-border">
      <div className="flex items-center gap-1.5 text-caiman-dim mb-1">
        <Icon size={10} />
        <span className="text-[8px] uppercase tracking-[1.5px]">{label}</span>
      </div>
      <span className={clsx('text-[16px] font-mono font-light', color || 'text-[#e8f5e9]')}>
        {value}
      </span>
    </div>
  )
}

export function VmDetailPanel() {
  const { detailOpen, toggleDetail, selectedVmId } = useClusterStore(s => ({
    detailOpen:    s.detailOpen,
    toggleDetail:  s.toggleDetail,
    selectedVmId:  s.selectedVmId,
  }))
  const vm = useClusterStore(selectSelectedVm)
  const [consoleOpen, setConsoleOpen] = React.useState(false)
  const [loading, setLoading] = React.useState(false)

  const handleStart = async () => {
    if (!vm || loading) return
    setLoading(true)
    try {
      await startVm(vm.id)
      toast.success(`${vm.name} starting...`)
    } catch (e: any) {
      toast.error(`Failed to start: ${e.message}`)
    } finally {
      setLoading(false)
    }
  }

  const handleStop = async () => {
    if (!vm || loading) return
    setLoading(true)
    try {
      await stopVm(vm.id)
      toast.success(`${vm.name} stopped`)
    } catch (e: any) {
      toast.error(`Failed to stop: ${e.message}`)
    } finally {
      setLoading(false)
    }
  }

  const [confirmDelete, setConfirmDelete] = React.useState(false)

  const handleDelete = async () => {
    if (!vm || loading) return
    setLoading(true)
    try {
      await deleteVm(vm.id)
      toast.success(`${vm.name} deleted`)
      setConfirmDelete(false)
      toggleDetail(false)
    } catch (e: any) {
      toast.error(`Failed to delete: ${e.message}`)
    } finally {
      setLoading(false)
    }
  }

  const formatUptime = (secs: number) => {
    if (!secs) return '—'
    const d = Math.floor(secs / 86400)
    const h = Math.floor((secs % 86400) / 3600)
    const m = Math.floor((secs % 3600) / 60)
    if (d > 0) return `${d}d ${h}h`
    if (h > 0) return `${h}h ${m}m`
    return `${m}m`
  }

  return (
    <AnimatePresence>
      {detailOpen && vm && (
        <motion.aside
          initial={{ x: '100%' }}
          animate={{ x: 0 }}
          exit={{ x: '100%' }}
          transition={{ type: 'spring', stiffness: 300, damping: 30 }}
          className="absolute top-0 right-0 bottom-0 w-[280px] bg-caiman-bg4
                     border-l border-caiman-border flex flex-col z-20 shadow-panel"
        >
          {/* Header */}
          <div className="flex items-start justify-between px-3 py-2.5
                          border-b border-caiman-border flex-shrink-0">
            <div>
              <div className="text-[12px] text-[#e8f5e9] font-mono font-medium truncate max-w-[200px]">
                {vm.name}
              </div>
              <div className="text-[8px] text-caiman-dim mt-0.5 flex items-center gap-2">
                <span>#{vm.id}</span>
                <span>·</span>
                <span className="text-caiman-green">{vm.nodeName}</span>
              </div>
            </div>
            <button onClick={() => toggleDetail(false)} className="text-caiman-dim hover:text-caiman-bright p-0.5">
              <X size={13} />
            </button>
          </div>

          {/* Body */}
          <div className="flex-1 overflow-y-auto p-3 flex flex-col gap-3">

            {/* Status */}
            <div className="flex items-center justify-between">
              <StatusPill status={vm.status} />
              <span className="text-[9px] text-caiman-dim">Up {formatUptime(vm.uptimeSecs)}</span>
            </div>

            {/* Migration progress */}
            {vm.migrating && (
              <div className="bg-[#0a1f2e] border border-[#1565c0] rounded-md p-2.5">
                <div className="flex justify-between text-[9px] mb-1.5">
                  <span className="text-caiman-blue tracking-wide">MIGRATING</span>
                  <span className="text-caiman-blue">{vm.migrating.progressPct.toFixed(0)}%</span>
                </div>
                <div className="h-1 bg-[#0a1525] rounded-full overflow-hidden">
                  <div
                    className="h-full bg-caiman-blue rounded-full transition-all duration-500"
                    style={{ width: `${vm.migrating.progressPct}%` }}
                  />
                </div>
                <div className="text-[8px] text-caiman-dim mt-1.5">
                  {vm.migrating.fromNode} → {vm.migrating.toNode}
                </div>
                <div className="text-[8px] text-caiman-dim">Phase: {vm.migrating.phase}</div>
              </div>
            )}

            {/* Stats grid */}
            <div className="grid grid-cols-2 gap-1.5">
              <MiniStat
                icon={Cpu} label="CPU"
                value={`${vm.cpuUsagePct.toFixed(0)}%`}
                color={vm.cpuUsagePct > 80 ? 'text-caiman-red' : vm.cpuUsagePct > 60 ? 'text-caiman-amber' : 'text-caiman-bright'}
              />
              <MiniStat
                icon={MemoryStick} label="RAM"
                value={`${Math.round(vm.memMib / 1024)}/${Math.round(vm.memTotalMib / 1024)}G`}
              />
              <MiniStat icon={Network}   label="RX"  value={`${vm.netRxMbps.toFixed(1)} Gb`} color="text-caiman-bright" />
              <MiniStat icon={Network}   label="TX"  value={`${vm.netTxMbps.toFixed(1)} Gb`} />
            </div>

            {/* Details */}
            <div>
              <div className="sec-label">Details</div>
              {[
                ['Node',    vm.nodeName],
                ['vCPUs',   vm.cpuCores.toString()],
                ['MAC',     vm.mac],
                ['GPU',     vm.gpuAlloc?.profile ?? '—'],
                ['Started', vm.startedAt.split('T')[0] ?? '—'],
              ].map(([k, v]) => (
                <div key={k} className="flex justify-between py-1 border-b border-caiman-bg
                                        text-[9px] last:border-b-0">
                  <span className="text-caiman-dim">{k}</span>
                  <span className="text-caiman-text font-mono">{v}</span>
                </div>
              ))}
            </div>

            {/* Labels */}
            <div>
              <div className="sec-label">Labels</div>
              <div className="flex flex-wrap gap-1">
                {Object.entries(vm.labels).map(([k, v]) => (
                  <span key={k} className={clsx(
                    'tag',
                    k === 'env' && v === 'prod' ? 'tag-green' : '',
                    k === 'role' && v === 'primary' ? 'tag-amber' : '',
                  )}>
                    {k}={v}
                  </span>
                ))}
              </div>
            </div>

            {/* Console preview */}
            <div>
              <div className="sec-label">Serial console</div>
              <div className="bg-caiman-bg border border-caiman-border rounded p-2
                              font-mono text-[8px] text-caiman-green h-[80px]
                              overflow-hidden leading-relaxed">
                <div>[ 0.000000] Booting Linux 6.6.29</div>
                <div>[ 0.412831] caiman_net: loaded OK</div>
                <div>[ 0.413100] XDP: attached on eth0</div>
                <div>[ 1.204310] virtio-net: link up</div>
                <div>[ 2.001124] Reached target: Network</div>
              </div>
            </div>
          </div>

          {/* Actions */}
          <div className="p-2.5 border-t border-caiman-border flex flex-wrap gap-1.5">
            <button className="btn-primary flex-1 text-[9px]" onClick={() => setConsoleOpen(true)}
              disabled={vm.status !== 'RUNNING'}
              title={vm.status !== 'RUNNING' ? 'Start the VM first' : 'Open console'}>
              <Terminal size={10} /> Console
            </button>
            <button className="btn flex-1 text-[9px]">
              <RefreshCw size={10} /> Migrate
            </button>
            {vm.status === 'RUNNING' && (
              <button className="btn btn-danger flex-1 text-[9px]" onClick={handleStop} disabled={loading}>
                <StopCircle size={10} /> {loading ? "..." : "Stop"}
              </button>
            )}
            {vm.status === 'STOPPED' && (
              <button className="btn flex-1 text-[9px] border-caiman-border2 text-caiman-bright" onClick={handleStart} disabled={loading}>
                <Play size={10} /> {loading ? "..." : "Start"}
              </button>
            )}
            <button
              className="btn flex-1 text-[9px] border-red-900/40 text-caiman-red hover:bg-red-900/20"
              onClick={() => setConfirmDelete(true)}
              disabled={loading}
              title="Delete VM permanently"
            >
              <Trash2 size={10} /> Delete
            </button>
          </div>
          {confirmDelete && (
            <div className="absolute inset-0 z-30 bg-black/80 flex items-center justify-center p-4">
              <div className="bg-caiman-bg2 border border-caiman-red/50 rounded-lg p-4 w-full max-w-[260px] shadow-xl">
                <div className="flex items-center gap-2 mb-3">
                  <AlertTriangle size={14} className="text-caiman-red" />
                  <span className="text-[11px] text-[#e8f5e9] font-mono tracking-wide">Delete VM?</span>
                </div>
                <div className="text-[10px] text-caiman-dim mb-3 leading-relaxed">
                  This will permanently delete <span className="text-caiman-text font-mono">{vm.name}</span> and its disk. This cannot be undone.
                </div>
                <div className="flex gap-2">
                  <button
                    className="flex-1 text-[10px] tracking-[1.5px] uppercase py-1.5 rounded border border-caiman-border text-caiman-dim hover:text-caiman-text"
                    onClick={() => setConfirmDelete(false)}
                    disabled={loading}
                  >Cancel</button>
                  <button
                    className="flex-1 text-[10px] tracking-[1.5px] uppercase py-1.5 rounded bg-red-900/30 border border-caiman-red/50 text-caiman-red hover:bg-red-900/50"
                    onClick={handleDelete}
                    disabled={loading}
                  >{loading ? "..." : "Delete"}</button>
                </div>
              </div>
            </div>
          )}
        </motion.aside>
      )}
      {consoleOpen && vm && (
        <ConsoleModal vmId={vm.id} vmName={vm.name} onClose={() => setConsoleOpen(false)} />
      )}
    </AnimatePresence>
  )
}

function StatusPill({ status }: { status: Vm['status'] }) {
  const map: Record<Vm['status'], string> = {
    RUNNING:   'pill-run',
    MIGRATING: 'pill-mig',
    STOPPED:   'pill-stop',
    BOOTING:   'pill-boot',
    ERROR:     'pill-crit',
  }
  const hasDot = status !== 'STOPPED' && status !== 'ERROR'
  return (
    <span className={`pill ${map[status]}`}>
      {hasDot && <span className="w-1 h-1 rounded-full bg-current animate-pulse-fast" />}
      {status}
    </span>
  )
}
