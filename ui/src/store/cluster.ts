import { create } from 'zustand'
import { immer } from 'zustand/middleware/immer'
import { subscribeWithSelector } from 'zustand/middleware'
import type {
  ClusterSnapshot, Vm, ClusterNode, DrsRecommendation,
  Notification, NotifLevel, AuditEvent, WsEvent,
} from '../types'

// ── Cluster store ─────────────────────────────────────────────────────────

interface ClusterState {
  snapshot:        ClusterSnapshot | null
  selectedVmId:    string | null
  selectedNodeId:  string | null
  drsRecs:         DrsRecommendation[]
  auditEvents:     AuditEvent[]
  notifications:   Notification[]
  unreadCount:     number
  sidebarOpen:     boolean
  detailOpen:      boolean
  commandBarOpen:  boolean
  liveMode:        boolean

  // Actions
  setSnapshot:     (s: ClusterSnapshot) => void
  applyWsEvent:    (e: WsEvent) => void
  selectVm:        (id: string | null) => void
  selectNode:      (id: string | null) => void
  openCommandBar:  () => void
  closeCommandBar: () => void
  toggleDetail:    (open?: boolean) => void
  addNotification: (level: NotifLevel, title: string, msg: string) => void
  markAllRead:     () => void
  clearNotif:      (id: string) => void
}

let notifCounter = 0

export const useClusterStore = create<ClusterState>()(
  subscribeWithSelector(
    immer((set) => ({
      snapshot:       null,
      selectedVmId:   null,
      selectedNodeId: null,
      drsRecs:        [],
      auditEvents:    [],
      notifications:  [],
      unreadCount:    0,
      sidebarOpen:    true,
      detailOpen:     false,
      commandBarOpen: false,
      liveMode:       true,

      setSnapshot: (s) => set((state) => {
        state.snapshot = s
      }),

      applyWsEvent: (event) => set((state) => {
        if (!state.snapshot) return

        switch (event.type) {
          case 'VM_METRICS_UPDATE': {
            const vm = state.snapshot.vms.find(v => v.id === event.payload.id)
            if (vm) Object.assign(vm, event.payload)
            // Recompute cluster totals
            const running = state.snapshot.vms.filter(v => v.status === 'RUNNING')
            state.snapshot.totalCpuPct = running.length
              ? running.reduce((s, v) => s + v.cpuUsagePct, 0) / running.length
              : 0
            state.snapshot.xdpThroughputGbps =
              running.reduce((s, v) => s + v.netRxMbps + v.netTxMbps, 0) / 1000
            break
          }
          case 'NODE_METRICS_UPDATE': {
            const node = state.snapshot.nodes.find(n => n.id === event.payload.id)
            if (node) Object.assign(node, event.payload)
            // Recompute sigma
            const scores = state.snapshot.nodes.map(n => n.loadScore)
            const mean = scores.reduce((s, v) => s + v, 0) / scores.length
            const variance = scores.reduce((s, v) => s + (v - mean) ** 2, 0) / scores.length
            state.snapshot.balanceSigma = Math.sqrt(variance)
            break
          }
          case 'VM_STATUS_CHANGE': {
            const vm = state.snapshot.vms.find(v => v.id === event.payload.id)
            if (vm) {
              vm.status = event.payload.status
              vm.migrating = event.payload.migrating
            }
            break
          }
          case 'DRS_RECOMMENDATION':
            state.drsRecs = [event.payload, ...state.drsRecs.slice(0, 9)]
            break
          case 'MICROSEG_DENY':
            state.auditEvents = [event.payload, ...state.auditEvents.slice(0, 99)]
            break
          case 'ALERT':
            state.notifications.unshift(event.payload)
            state.unreadCount++
            break
          case 'MIGRATION_PROGRESS': {
            const vm = state.snapshot.vms.find(v => v.id === event.payload.vmId)
            if (vm?.migrating) {
              vm.migrating.phase = event.payload.phase
              vm.migrating.progressPct = event.payload.progressPct
            }
            break
          }
        }
      }),

      selectVm: (id) => set((state) => {
        state.selectedVmId  = id
        state.detailOpen    = id !== null
      }),

      selectNode: (id) => set((state) => {
        state.selectedNodeId = id
        state.detailOpen     = id !== null
      }),

      openCommandBar:  () => set((s) => { s.commandBarOpen = true }),
      closeCommandBar: () => set((s) => { s.commandBarOpen = false }),
      toggleDetail:    (open) => set((s) => { s.detailOpen = open ?? !s.detailOpen }),

      addNotification: (level, title, message) => set((state) => {
        state.notifications.unshift({
          id:        `n-${++notifCounter}`,
          level, title, message,
          timestamp: Date.now(),
          read:      false,
        })
        state.unreadCount++
        // Keep last 50
        if (state.notifications.length > 50) {
          state.notifications.pop()
        }
      }),

      markAllRead: () => set((state) => {
        state.notifications.forEach(n => { n.read = true })
        state.unreadCount = 0
      }),

      clearNotif: (id) => set((state) => {
        const idx = state.notifications.findIndex(n => n.id === id)
        if (idx !== -1) {
          if (!state.notifications[idx].read) state.unreadCount--
          state.notifications.splice(idx, 1)
        }
      }),
    }))
  )
)

// ── Selectors ─────────────────────────────────────────────────────────────

export const selectVm = (id: string) => (s: ClusterState) =>
  s.snapshot?.vms.find(v => v.id === id)

export const selectNode = (id: string) => (s: ClusterState) =>
  s.snapshot?.nodes.find(n => n.id === id)

export const selectRunningVms = (s: ClusterState) =>
  s.snapshot?.vms.filter(v => v.status === 'RUNNING') ?? []

export const selectSelectedVm = (s: ClusterState) =>
  s.snapshot?.vms.find(v => v.id === s.selectedVmId)
