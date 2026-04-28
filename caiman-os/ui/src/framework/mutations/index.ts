/**
 * framework/mutations/index.ts
 *
 * All API mutations using TanStack Query's useMutation.
 * Each mutation handles:
 *   - optimistic UI update (instant feedback)
 *   - real API call via caiman-api
 *   - WebSocket-triggered invalidation (no polling needed)
 *   - error rollback
 *   - toast notification on success/failure
 */

import { useMutation, useQueryClient } from '@tanstack/react-query'
import { api } from '../../api/client'
import { useClusterStore } from '../../store/cluster'
import toast from 'react-hot-toast'
import type { VmRef, MigrateInput, CreateVmInput, ExecuteDrsInput } from '../actions/registry'

// ── Query keys (single source of truth) ──────────────────────────────────

export const KEYS = {
  cluster:      ['cluster'],
  nodes:        ['nodes'],
  vms:          ['vms'],
  vm:     (id: string) => ['vms', id],
  console:(id: string) => ['vms', id, 'console'],
  drs:          ['drs', 'recommendations'],
  xdp:          ['xdp', 'stats'],
  microseg:     ['microseg', 'policies'],
  audit:        ['microseg', 'audit'],
  storage:      ['storage', 'vsan'],
  gpu:          ['gpu', 'allocations'],
} as const

// ── VM mutations ──────────────────────────────────────────────────────────

export function useStartVm() {
  const qc = useQueryClient()
  const store = useClusterStore.getState()

  return useMutation({
    mutationFn: ({ vmId }: VmRef) =>
      api.post(`/api/vms/${vmId}/start`).then(r => r.data),

    onMutate: async ({ vmId, vmName }) => {
      // Optimistic: mark as BOOTING
      await qc.cancelQueries({ queryKey: KEYS.vms })
      const prev = qc.getQueryData(KEYS.vms)
      qc.setQueryData(KEYS.vms, (old: any[]) =>
        old?.map(v => v.id === vmId ? { ...v, status: 'BOOTING' } : v) ?? []
      )
      store.addNotification('info', 'Starting', `Starting ${vmName}…`)
      return { prev }
    },

    onSuccess: (_, { vmName }) => {
      qc.invalidateQueries({ queryKey: KEYS.cluster })
      toast.success(`${vmName} started`, { icon: '▶' })
    },

    onError: (err, { vmName }, ctx) => {
      if (ctx?.prev) qc.setQueryData(KEYS.vms, ctx.prev)
      toast.error(`Failed to start ${vmName}`)
      store.addNotification('error', 'Start failed', String(err))
    },
  })
}

export function useStopVm() {
  const qc = useQueryClient()
  const store = useClusterStore.getState()

  return useMutation({
    mutationFn: ({ vmId }: VmRef) =>
      api.post(`/api/vms/${vmId}/stop`).then(r => r.data),

    onMutate: async ({ vmId }) => {
      const prev = qc.getQueryData(KEYS.vms)
      qc.setQueryData(KEYS.vms, (old: any[]) =>
        old?.map(v => v.id === vmId ? { ...v, status: 'STOPPED' } : v) ?? []
      )
      return { prev }
    },

    onSuccess: (_, { vmName }) => {
      qc.invalidateQueries({ queryKey: KEYS.cluster })
      toast.success(`${vmName} stopped`)
    },

    onError: (err, { vmName }, ctx) => {
      if (ctx?.prev) qc.setQueryData(KEYS.vms, ctx.prev)
      toast.error(`Failed to stop ${vmName}`)
    },
  })
}

export function useMigrateVm() {
  const qc    = useQueryClient()
  const store = useClusterStore.getState()

  return useMutation({
    mutationFn: ({ vmId, toNode }: MigrateInput) =>
      api.post(`/api/vms/${vmId}/migrate`, { toNode }).then(r => r.data),

    onMutate: async ({ vmId, vmName, toNode }) => {
      const prev = qc.getQueryData(KEYS.vms)
      qc.setQueryData(KEYS.vms, (old: any[]) =>
        old?.map(v => v.id === vmId ? {
          ...v,
          status: 'MIGRATING',
          migrating: { phase: 'Setup', fromNode: v.nodeName, toNode, progressPct: 0, elapsedSecs: 0 },
        } : v) ?? []
      )
      store.addNotification('info', 'Migration started',
        `Migrating ${vmName} → ${toNode}`)
      return { prev }
    },

    onSuccess: (_, { vmName, toNode }) => {
      // Don't invalidate immediately — WebSocket will push status changes
      store.addNotification('success', 'Migration complete',
        `${vmName} migrated to ${toNode}`)
    },

    onError: (err, { vmName }, ctx) => {
      if (ctx?.prev) qc.setQueryData(KEYS.vms, ctx.prev)
      toast.error(`Migration failed: ${String(err)}`)
      store.addNotification('error', 'Migration failed', String(err))
    },
  })
}

export function useCreateVm() {
  const qc = useQueryClient()

  return useMutation({
    mutationFn: (input: CreateVmInput) =>
      api.post('/api/vms', input).then(r => r.data),

    onSuccess: (data) => {
      qc.invalidateQueries({ queryKey: KEYS.cluster })
      toast.success(`VM created: ${data.id}`, { icon: '🐊' })
    },

    onError: (err) => toast.error(`Create VM failed: ${String(err)}`),
  })
}

export function useDeleteVm() {
  const qc    = useQueryClient()
  const store = useClusterStore.getState()

  return useMutation({
    mutationFn: ({ vmId }: VmRef) =>
      api.delete(`/api/vms/${vmId}`).then(r => r.data),

    onMutate: async ({ vmId }) => {
      const prev = qc.getQueryData(KEYS.vms)
      qc.setQueryData(KEYS.vms,
        (old: any[]) => old?.filter(v => v.id !== vmId) ?? []
      )
      return { prev }
    },

    onSuccess: (_, { vmName }) => {
      qc.invalidateQueries({ queryKey: KEYS.cluster })
      toast.success(`${vmName} deleted`)
    },

    onError: (err, { vmName }, ctx) => {
      if (ctx?.prev) qc.setQueryData(KEYS.vms, ctx.prev)
      toast.error(`Delete failed: ${String(err)}`)
    },
  })
}

// ── DRS mutations ─────────────────────────────────────────────────────────

export function useExecuteDrs() {
  const qc    = useQueryClient()
  const store = useClusterStore.getState()

  return useMutation({
    mutationFn: ({ vmId }: ExecuteDrsInput) =>
      api.post(`/api/drs/execute/${vmId}`).then(r => r.data),

    onMutate: ({ vmId, fromNode, toNode }) => {
      store.addNotification('info', 'DRS executing',
        `Migrating ${vmId}: ${fromNode} → ${toNode}`)
    },

    onSuccess: () => {
      qc.invalidateQueries({ queryKey: KEYS.drs })
      toast.success('DRS migration started', { icon: '⚡' })
    },

    onError: (err) => toast.error(`DRS execute failed: ${String(err)}`),
  })
}

export function useExecuteAllDrs() {
  const qc = useQueryClient()

  return useMutation({
    mutationFn: () => api.post('/api/drs/execute-all').then(r => r.data),
    onSuccess:  () => {
      qc.invalidateQueries({ queryKey: KEYS.drs })
      toast.success('All DRS migrations started')
    },
    onError: (err) => toast.error(`DRS execute-all failed: ${String(err)}`),
  })
}

export function useSetDrsMode() {
  const qc = useQueryClient()

  return useMutation({
    mutationFn: ({ mode }: { mode: string }) =>
      api.patch('/api/drs/config', { mode }).then(r => r.data),

    onMutate: ({ mode }) => {
      // Optimistic update cluster DRS mode label
      qc.setQueryData(KEYS.cluster, (old: any) =>
        old ? { ...old, drsMode: mode } : old
      )
    },

    onSuccess: (_, { mode }) => {
      qc.invalidateQueries({ queryKey: KEYS.cluster })
      toast.success(`DRS mode: ${mode}`)
    },

    onError: (err) => toast.error(`Set DRS mode failed: ${String(err)}`),
  })
}

// ── Micro-seg mutations ───────────────────────────────────────────────────

export function useCreatePolicy() {
  const qc = useQueryClient()

  return useMutation({
    mutationFn: (policy: unknown) =>
      api.post('/api/microseg/policies', policy).then(r => r.data),

    onSuccess: (data: any) => {
      qc.invalidateQueries({ queryKey: KEYS.microseg })
      toast.success(`Policy "${data.name}" created — XDP enforcing`, { icon: '🛡' })
    },

    onError: (err) => toast.error(`Policy create failed: ${String(err)}`),
  })
}

export function useDeletePolicy() {
  const qc = useQueryClient()

  return useMutation({
    mutationFn: ({ name, namespace }: { name: string; namespace: string }) =>
      api.delete(`/api/microseg/policies/${namespace}/${name}`).then(r => r.data),

    onMutate: async ({ name }) => {
      const prev = qc.getQueryData(KEYS.microseg)
      qc.setQueryData(KEYS.microseg, (old: any) => ({
        ...old,
        policies: old?.policies?.filter((p: any) => p.name !== name) ?? [],
      }))
      return { prev }
    },

    onSuccess: (_, { name }) => {
      qc.invalidateQueries({ queryKey: KEYS.microseg })
      toast.success(`Policy "${name}" deleted`)
    },

    onError: (err, _, ctx) => {
      if (ctx?.prev) qc.setQueryData(KEYS.microseg, ctx.prev)
      toast.error(`Delete policy failed: ${String(err)}`)
    },
  })
}

// ── Storage mutations ─────────────────────────────────────────────────────

export function useCreateVsanVolume() {
  const qc = useQueryClient()

  return useMutation({
    mutationFn: (input: { name: string; sizeGib: number; ftt: number }) =>
      api.post('/api/storage/vsan', input).then(r => r.data),

    onSuccess: (data: any) => {
      qc.invalidateQueries({ queryKey: KEYS.storage })
      toast.success(`Volume "${data.name}" created (${data.sizeGib} GiB)`)
    },

    onError: (err) => toast.error(`Volume create failed: ${String(err)}`),
  })
}
