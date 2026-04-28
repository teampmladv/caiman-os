/**
 * framework/progress/index.tsx
 *
 * Tracks long-running operations triggered by the ActionBus.
 * Shown as a persistent panel at the bottom of the UI when any
 * operation is in progress (live migration, VM boot, volume creation).
 *
 * Data sources:
 *   - WebSocket MigrationProgress events (phase + progressPct)
 *   - Polling /api/vms/:id for status changes
 *   - ActionBus onSuccess / onError callbacks
 */

import React, { useEffect } from 'react'
import { create }  from 'zustand'
import { immer }   from 'zustand/middleware/immer'
import { motion, AnimatePresence } from 'framer-motion'
import { X, Loader2, CheckCircle, AlertCircle } from 'lucide-react'
import { clsx } from 'clsx'

// ── Progress store ────────────────────────────────────────────────────────

export type OpStatus = 'running' | 'success' | 'error'

export interface TrackedOp {
  id:          string
  actionId:    string
  label:       string
  detail:      string
  status:      OpStatus
  progressPct: number
  phase:       string
  startedAt:   number
  elapsedMs:   number
  estimatedMs: number
}

interface ProgressStore {
  ops:    TrackedOp[]
  addOp:  (op: Omit<TrackedOp, 'startedAt' | 'elapsedMs'>) => void
  updateOp: (id: string, update: Partial<TrackedOp>) => void
  removeOp: (id: string) => void
  clearDone: () => void
}

export const useProgressStore = create<ProgressStore>()(
  immer((set) => ({
    ops: [],

    addOp: (op) => set((s) => {
      s.ops.push({ ...op, startedAt: Date.now(), elapsedMs: 0 })
    }),

    updateOp: (id, update) => set((s) => {
      const op = s.ops.find(o => o.id === id)
      if (op) {
        Object.assign(op, update)
        op.elapsedMs = Date.now() - op.startedAt
      }
    }),

    removeOp: (id) => set((s) => {
      s.ops = s.ops.filter(o => o.id !== id)
    }),

    clearDone: () => set((s) => {
      s.ops = s.ops.filter(o => o.status === 'running')
    }),
  }))
)

// ── Hook: track a VM migration from WebSocket events ─────────────────────

export function useMigrationTracker(vmId: string) {
  const { addOp, updateOp } = useProgressStore()

  return {
    start: (fromNode: string, toNode: string, estimatedMs: number) => {
      addOp({
        id:          `mig-${vmId}`,
        actionId:    'vm.migrate',
        label:       `Migrating ${vmId}`,
        detail:      `${fromNode} → ${toNode}`,
        status:      'running',
        progressPct: 0,
        phase:       'Setup',
        estimatedMs,
      })
    },
    update: (phase: string, progressPct: number) => {
      updateOp(`mig-${vmId}`, { phase, progressPct })
    },
    complete: (blackoutMs?: number) => {
      updateOp(`mig-${vmId}`, {
        status:      'success',
        progressPct: 100,
        phase:       `Done ${blackoutMs ? `(${blackoutMs}ms blackout)` : ''}`,
      })
      setTimeout(() => useProgressStore.getState().removeOp(`mig-${vmId}`), 6000)
    },
    fail: (reason: string) => {
      updateOp(`mig-${vmId}`, { status: 'error', phase: reason })
    },
  }
}

// ── Progress panel UI ─────────────────────────────────────────────────────

export function ProgressPanel() {
  const { ops, removeOp, clearDone } = useProgressStore()
  const running = ops.filter(o => o.status === 'running').length

  if (ops.length === 0) return null

  return (
    <motion.div
      initial={{ y: 60, opacity: 0 }}
      animate={{ y: 0,  opacity: 1 }}
      exit={{    y: 60, opacity: 0 }}
      className="fixed bottom-8 left-3 w-[320px] bg-caiman-bg3 border border-caiman-border
                 rounded-xl shadow-panel overflow-hidden z-40"
    >
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-2
                      border-b border-caiman-border bg-caiman-bg2">
        <div className="flex items-center gap-2">
          {running > 0 && (
            <Loader2 size={11} className="text-caiman-bright animate-spin" />
          )}
          <span className="text-[9px] text-caiman-dim tracking-[2px] uppercase">
            {running > 0 ? `${running} operation${running > 1 ? 's' : ''} in progress` : 'Operations'}
          </span>
        </div>
        <button onClick={clearDone} className="text-caiman-dim hover:text-caiman-text">
          <X size={11} />
        </button>
      </div>

      {/* Op list */}
      <div className="divide-y divide-caiman-border max-h-[240px] overflow-y-auto">
        <AnimatePresence initial={false}>
          {ops.map(op => (
            <motion.div
              key={op.id}
              initial={{ height: 0, opacity: 0 }}
              animate={{ height: 'auto', opacity: 1 }}
              exit={{    height: 0, opacity: 0 }}
              transition={{ duration: 0.15 }}
              className="px-3 py-2.5"
            >
              <div className="flex items-center justify-between mb-1.5">
                <div className="flex items-center gap-1.5">
                  <StatusIcon status={op.status} />
                  <span className="text-[10px] text-[#e8f5e9] font-medium">{op.label}</span>
                </div>
                {op.status !== 'running' && (
                  <button onClick={() => removeOp(op.id)}
                          className="text-caiman-dim hover:text-caiman-text">
                    <X size={9} />
                  </button>
                )}
              </div>

              {/* Progress bar */}
              {op.status === 'running' && (
                <div className="bar-track mb-1">
                  <motion.div
                    className="bar-fill bg-caiman-bright"
                    style={{ width: `${op.progressPct}%` }}
                    transition={{ duration: 0.5 }}
                  />
                </div>
              )}

              {/* Phase + detail */}
              <div className="flex justify-between text-[8px] text-caiman-dim">
                <span>{op.phase}</span>
                <span>{op.detail}</span>
              </div>

              {/* Elapsed / estimated */}
              {op.status === 'running' && op.estimatedMs > 0 && (
                <div className="text-[8px] text-caiman-dim mt-0.5">
                  {formatMs(op.elapsedMs)} / ~{formatMs(op.estimatedMs)}
                </div>
              )}
            </motion.div>
          ))}
        </AnimatePresence>
      </div>
    </motion.div>
  )
}

function StatusIcon({ status }: { status: OpStatus }) {
  if (status === 'running')
    return <Loader2 size={11} className="text-caiman-bright animate-spin" />
  if (status === 'success')
    return <CheckCircle size={11} className="text-caiman-bright" />
  return <AlertCircle size={11} className="text-caiman-red" />
}

function formatMs(ms: number): string {
  if (ms < 1000) return `${ms}ms`
  if (ms < 60000) return `${(ms / 1000).toFixed(0)}s`
  return `${(ms / 60000).toFixed(1)}m`
}
