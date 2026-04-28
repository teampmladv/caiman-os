/**
 * framework/shortcuts/index.ts
 *
 * Global keyboard shortcut registry.
 * Shortcuts are bound to ActionIds — the ActionBus handles execution.
 *
 * Usage:
 *   useShortcut('vm.stop', () => bus.dispatch('vm.stop', { vmId, vmName }))
 *   // Or via the hook with automatic context (selected VM):
 *   useContextShortcuts()
 */

import { useEffect, useCallback } from 'react'
import { ACTIONS, type ActionId } from '../actions/registry'
import { useClusterStore, selectSelectedVm } from '../../store/cluster'
import { useActionBus } from '../bus'

// ── Parse shortcut string ─────────────────────────────────────────────────

interface ParsedShortcut {
  key:   string
  mod:   boolean
  shift: boolean
  alt:   boolean
}

function parseShortcut(s: string): ParsedShortcut {
  const parts = s.toLowerCase().split('+')
  return {
    key:   parts[parts.length - 1],
    mod:   parts.includes('mod'),
    shift: parts.includes('shift'),
    alt:   parts.includes('alt'),
  }
}

function matches(e: KeyboardEvent, parsed: ParsedShortcut): boolean {
  const mod = navigator.platform.includes('Mac') ? e.metaKey : e.ctrlKey
  return (
    e.key.toLowerCase() === parsed.key
    && mod   === parsed.mod
    && e.shiftKey === parsed.shift
    && e.altKey   === parsed.alt
  )
}

// ── Global shortcut hook ──────────────────────────────────────────────────

export function useShortcut(
  shortcut: string,
  callback: (e: KeyboardEvent) => void,
  deps: unknown[] = [],
) {
  const parsed = parseShortcut(shortcut)
  const stable = useCallback(callback, deps)

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      // Don't fire when typing in inputs
      const tag = (e.target as HTMLElement).tagName
      if (['INPUT', 'TEXTAREA'].includes(tag)) return
      if (matches(e, parsed)) {
        e.preventDefault()
        stable(e)
      }
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [parsed.key, parsed.mod, parsed.shift, parsed.alt, stable])
}

// ── Context-aware shortcuts (act on selected VM) ──────────────────────────

export function useContextShortcuts() {
  const { dispatch, busy }   = useActionBus()
  const selectedVm           = useClusterStore(selectSelectedVm)
  const { openCommandBar }   = useClusterStore(s => ({
    openCommandBar: s.openCommandBar,
  }))

  // ⌘K — command bar (already in TopNav, but also here as fallback)
  useShortcut('mod+k', () => openCommandBar())

  // Context shortcuts (require selected VM)
  useShortcut('mod+shift+s', () => {
    if (selectedVm?.status === 'STOPPED') {
      dispatch('vm.start', { vmId: selectedVm.id, vmName: selectedVm.name })
    }
  }, [selectedVm, dispatch])

  useShortcut('mod+shift+x', () => {
    if (selectedVm?.status === 'RUNNING') {
      dispatch('vm.stop', { vmId: selectedVm.id, vmName: selectedVm.name })
    }
  }, [selectedVm, dispatch])

  useShortcut('mod+m', () => {
    if (selectedVm) {
      // Open migrate dialog — handled by ⌘K or context menu
      openCommandBar()
    }
  }, [selectedVm, openCommandBar])

  useShortcut('mod+`', () => {
    if (selectedVm) {
      dispatch('vm.console', { vmId: selectedVm.id, vmName: selectedVm.name })
    }
  }, [selectedVm, dispatch])

  // DRS
  useShortcut('mod+shift+d', () => {
    dispatch('drs.execute-all', undefined)
  }, [dispatch])

  // New VM
  useShortcut('mod+shift+n', () => {
    // Opens create VM dialog — trigger via ⌘K
    openCommandBar()
  }, [openCommandBar])
}

// ── Shortcut help tooltip data ────────────────────────────────────────────

export const SHORTCUT_HELP = Object.values(ACTIONS)
  .filter((a: any) => a.shortcut)
  .map(a => ({
    label:    a.label,
    shortcut: (a as any).shortcut!,
    category: a.category,
  }))
  .sort((a, b) => a.category.localeCompare(b.category))
