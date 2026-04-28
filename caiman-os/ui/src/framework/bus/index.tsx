/**
 * framework/bus/index.tsx
 *
 * The ActionBus is the single entry point for executing any action.
 * Components never call mutations directly — they call bus.dispatch().
 *
 * Flow:
 *   1. Component calls bus.dispatch('vm.stop', { vmId, vmName })
 *   2. Bus looks up ActionDef → gets confirm level
 *   3. If confirm === 'none'     → execute immediately
 *      If confirm === 'toast'    → show undo toast, execute after 4s
 *      If confirm === 'dialog'   → show ConfirmDialog, execute on OK
 *      If confirm === 'critical' → show CriticalConfirmDialog (type name)
 *   4. Execute: call mutation + write to AuditLog + broadcast WsEvent
 *   5. Track progress for long-running ops via ProgressTracker
 */

import React, {
  createContext, useContext, useCallback,
  useState, useRef, ReactNode,
} from 'react'
import toast from 'react-hot-toast'
import { motion, AnimatePresence } from 'framer-motion'
import { AlertTriangle, X } from 'lucide-react'
import { ACTIONS, getAction, type ActionId } from '../actions/registry'
import {
  useStartVm, useStopVm, useMigrateVm, useDeleteVm, useCreateVm,
  useExecuteDrs, useExecuteAllDrs, useSetDrsMode,
  useCreatePolicy, useDeletePolicy,
  useCreateVsanVolume,
} from '../mutations'
import { useAuditLog } from '../audit'
import type { MigrateInput, VmRef, CreateVmInput, ExecuteDrsInput } from '../actions/registry'

// ── Bus context ───────────────────────────────────────────────────────────

interface DispatchOptions {
  onSuccess?: () => void
  onError?:   (err: unknown) => void
}

interface BusContext {
  dispatch: <TInput>(id: ActionId, input: TInput, opts?: DispatchOptions) => void
  busy:     Record<string, boolean>
}

const Ctx = createContext<BusContext | null>(null)

// ── Provider ──────────────────────────────────────────────────────────────

export function ActionBusProvider({ children }: { children: ReactNode }) {
  const [busy, setBusy]       = useState<Record<string, boolean>>({})
  const [dialog, setDialog]   = useState<DialogState | null>(null)
  const pendingRef             = useRef<(() => void) | null>(null)
  const { log }                = useAuditLog()

  // All mutations
  const startVm   = useStartVm()
  const stopVm    = useStopVm()
  const migrateVm = useMigrateVm()
  const deleteVm  = useDeleteVm()
  const createVm  = useCreateVm()
  const execDrs   = useExecuteDrs()
  const execAllDrs= useExecuteAllDrs()
  const setDrsMode= useSetDrsMode()
  const createPol = useCreatePolicy()
  const deletePol = useDeletePolicy()
  const createVol = useCreateVsanVolume()

  const execute = useCallback(
    <TInput,>(id: ActionId, input: TInput, opts?: DispatchOptions) => {
      const def = getAction(id)
      setBusy(b => ({ ...b, [id]: true }))

      const finish = (err?: unknown) => {
        setBusy(b => ({ ...b, [id]: false }))
        if (err) opts?.onError?.(err)
        else     opts?.onSuccess?.()
      }

      log({ actionId: id, input, timestamp: Date.now() })

      // Route to the right mutation
      const promise = (() => {
        switch (id) {
          case 'vm.start':   return startVm.mutateAsync(input as VmRef)
          case 'vm.stop':    return stopVm.mutateAsync(input as VmRef)
          case 'vm.migrate': return migrateVm.mutateAsync(input as MigrateInput)
          case 'vm.delete':  return deleteVm.mutateAsync(input as VmRef)
          case 'vm.create':  return createVm.mutateAsync(input as CreateVmInput)
          case 'drs.execute-one': return execDrs.mutateAsync(input as ExecuteDrsInput)
          case 'drs.execute-all': return execAllDrs.mutateAsync()
          case 'drs.set-mode':    return setDrsMode.mutateAsync(input as { mode: string })
          case 'microseg.create-policy': return createPol.mutateAsync(input)
          case 'microseg.delete-policy': return deletePol.mutateAsync(input as any)
          case 'storage.create-volume':  return createVol.mutateAsync(input as any)
          default:
            return Promise.reject(new Error(`Action "${id}" has no executor`))
        }
      })()

      promise.then(() => finish()).catch(err => finish(err))
    },
    [startVm, stopVm, migrateVm, deleteVm, createVm,
     execDrs, execAllDrs, setDrsMode, createPol, deletePol, createVol, log]
  )

  const dispatch = useCallback(
    <TInput,>(id: ActionId, input: TInput, opts?: DispatchOptions) => {
      const def = getAction(id)

      if (def.confirm === 'none') {
        execute(id, input, opts)
        return
      }

      if (def.confirm === 'toast') {
        // Show undo toast — execute after 4s
        const msg = def.confirmMsg?.(input) ?? def.label
        let cancelled = false

        toast((t) => (
          <div className="flex items-center gap-3">
            <span className="text-[11px] text-caiman-text">{msg}</span>
            <button
              className="text-[9px] px-2 py-1 rounded border border-caiman-border2
                         text-caiman-bright bg-[#0d2e0d] cursor-pointer"
              onClick={() => { cancelled = true; toast.dismiss(t.id) }}
            >
              UNDO
            </button>
          </div>
        ), { duration: 4000, style: { background: '#0a150a', border: '1px solid #2e7d32' } })

        setTimeout(() => {
          if (!cancelled) execute(id, input, opts)
        }, 4000)
        return
      }

      if (def.confirm === 'dialog' || def.confirm === 'critical') {
        setDialog({
          id, input, opts,
          level:       def.confirm,
          message:     def.confirmMsg?.(input) ?? def.label,
          confirmWord: def.confirmWord?.(input),
          label:       def.label,
          icon:        def.icon,
        })
        pendingRef.current = () => execute(id, input, opts)
        return
      }
    },
    [execute]
  )

  const handleConfirm = useCallback(() => {
    pendingRef.current?.()
    setDialog(null)
    pendingRef.current = null
  }, [])

  const handleCancel = useCallback(() => {
    setDialog(null)
    pendingRef.current = null
  }, [])

  return (
    <Ctx.Provider value={{ dispatch, busy }}>
      {children}
      <ConfirmModal
        state={dialog}
        onConfirm={handleConfirm}
        onCancel={handleCancel}
      />
    </Ctx.Provider>
  )
}

// ── Hook ──────────────────────────────────────────────────────────────────

export function useActionBus() {
  const ctx = useContext(Ctx)
  if (!ctx) throw new Error('useActionBus must be inside ActionBusProvider')
  return ctx
}

// ── Convenience hook: bound action ────────────────────────────────────────

export function useAction<TInput>(id: ActionId) {
  const { dispatch, busy } = useActionBus()
  return {
    exec:    (input: TInput, opts?: DispatchOptions) => dispatch(id, input, opts),
    loading: busy[id] ?? false,
    def:     getAction(id),
  }
}

// ── Confirm modal ─────────────────────────────────────────────────────────

interface DialogState {
  id:           ActionId
  input:        unknown
  opts?:        DispatchOptions
  level:        'dialog' | 'critical'
  message:      string
  confirmWord?: string
  label:        string
  icon:         React.ElementType
}

function ConfirmModal({
  state,
  onConfirm,
  onCancel,
}: {
  state:     DialogState | null
  onConfirm: () => void
  onCancel:  () => void
}) {
  const [typed, setTyped] = React.useState('')
  const isCritical        = state?.level === 'critical'
  const canConfirm        = !isCritical || typed === state?.confirmWord

  React.useEffect(() => {
    if (!state) setTyped('')
  }, [state])

  return (
    <AnimatePresence>
      {state && (
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          className="fixed inset-0 bg-black/70 flex items-center justify-center z-[100]"
          onClick={onCancel}
        >
          <motion.div
            initial={{ scale: 0.95, opacity: 0, y: 8 }}
            animate={{ scale: 1,    opacity: 1, y: 0 }}
            exit={{    scale: 0.95, opacity: 0, y: 8 }}
            transition={{ type: 'spring', stiffness: 350, damping: 28 }}
            className="bg-caiman-bg3 border border-caiman-border rounded-xl p-6
                       w-[420px] shadow-panel"
            onClick={e => e.stopPropagation()}
          >
            {/* Icon + title */}
            <div className="flex items-start gap-3 mb-4">
              <div className={`w-9 h-9 rounded-lg flex items-center justify-center flex-shrink-0 ${
                isCritical ? 'bg-[#2a0000] text-caiman-red' : 'bg-[#1a1000] text-caiman-amber'
              }`}>
                <AlertTriangle size={16} />
              </div>
              <div>
                <div className="text-[13px] font-medium text-[#e8f5e9] mb-1">
                  {state.label}
                </div>
                <div className="text-[11px] text-caiman-muted leading-relaxed">
                  {state.message}
                </div>
              </div>
            </div>

            {/* Critical: type the resource name */}
            {isCritical && state.confirmWord && (
              <div className="mb-4">
                <div className="text-[9px] text-caiman-dim tracking-[1.5px] uppercase mb-1.5">
                  Type <span className="text-caiman-red font-medium">{state.confirmWord}</span> to confirm
                </div>
                <input
                  autoFocus
                  value={typed}
                  onChange={e => setTyped(e.target.value)}
                  className="input w-full font-mono"
                  placeholder={state.confirmWord}
                  onKeyDown={e => { if (e.key === 'Enter' && canConfirm) onConfirm() }}
                />
              </div>
            )}

            {/* Actions */}
            <div className="flex gap-2 justify-end">
              <button className="btn" onClick={onCancel}>
                Cancel
              </button>
              <button
                className={`btn ${canConfirm
                  ? (isCritical ? 'border-caiman-red text-caiman-red hover:bg-[#1a0d0d]' : 'btn-primary')
                  : 'opacity-40 cursor-not-allowed'}`}
                onClick={canConfirm ? onConfirm : undefined}
                disabled={!canConfirm}
              >
                {isCritical ? 'Delete permanently' : 'Confirm'}
              </button>
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  )
}
