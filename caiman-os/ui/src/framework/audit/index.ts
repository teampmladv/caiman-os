/**
 * framework/audit/index.ts
 *
 * Client-side audit log: every action dispatched through the ActionBus
 * is written here. The log is persisted to localStorage and synced to
 * caiman-api POST /api/audit (which stores in SQLite).
 *
 * This gives operators a complete history of "who did what when" in the UI,
 * separate from the caiman_net XDP audit events (which track packets).
 */

import { create } from 'zustand'
import { immer }  from 'zustand/middleware/immer'
import { persist } from 'zustand/middleware'
import { api }    from '../../api/client'
import type { ActionId } from '../actions/registry'

// ── Log entry ─────────────────────────────────────────────────────────────

export interface AuditEntry {
  id:        string
  actionId:  ActionId
  input:     unknown
  timestamp: number
  user:      string
  result?:   'success' | 'error'
  error?:    string
}

// ── Store ─────────────────────────────────────────────────────────────────

interface AuditStore {
  entries:  AuditEntry[]
  log:     (entry: Omit<AuditEntry, 'id' | 'user'>) => void
  resolve:  (id: string, result: 'success' | 'error', error?: string) => void
  clear:   () => void
}

let counter = 0

export const useAuditLog = create<AuditStore>()(
  persist(
    immer((set, get) => ({
      entries: [],

      log: (entry) => set((s) => {
        const id = `audit-${Date.now()}-${++counter}`
        const full: AuditEntry = {
          ...entry,
          id,
          user: localStorage.getItem('caiman-user') ?? 'operator',
        }
        s.entries.unshift(full)
        if (s.entries.length > 500) s.entries.pop()

        // Async sync to server (best-effort, don't block UI)
        api.post('/api/audit', full).catch(() => {/* ignore */})
      }),

      resolve: (id, result, error) => set((s) => {
        const entry = s.entries.find(e => e.id === id)
        if (entry) { entry.result = result; entry.error = error }
      }),

      clear: () => set((s) => { s.entries = [] }),
    })),
    {
      name:    'caiman-audit-log',
      version: 1,
      // Only persist last 100 entries to localStorage
      partialize: (s) => ({ entries: s.entries.slice(0, 100) }),
    }
  )
)
