/**
 * framework/index.ts — Public API of the Caimán action framework
 *
 * Everything a component needs to execute actions:
 *
 *   import { useAction, useActionBus, ActionButton } from '@/framework'
 *
 *   // Execute a specific action
 *   const stop = useAction<VmRef>('vm.stop')
 *   <button onClick={() => stop.exec({ vmId, vmName })} disabled={stop.loading}>
 *     Stop
 *   </button>
 *
 *   // Or via the bus directly (useful for dynamic action IDs)
 *   const { dispatch } = useActionBus()
 *   dispatch('vm.migrate', { vmId, vmName, toNode })
 *
 *   // Or use the pre-built ActionButton component
 *   <ActionButton actionId="vm.stop" input={{ vmId, vmName }} />
 */

export { ActionBusProvider, useActionBus, useAction } from './bus'
export { useProgressStore, ProgressPanel, useMigrationTracker } from './progress'
export { useContextShortcuts, useShortcut, SHORTCUT_HELP } from './shortcuts'
export { useAuditLog }   from './audit'
export { ACTIONS, getAction, searchActions, getActionsByCategory } from './actions/registry'
export * from './mutations'
export type { ActionId, ActionDef, ActionCategory } from './actions/registry'

// ── Pre-built ActionButton ────────────────────────────────────────────────

import React from 'react'
import { clsx } from 'clsx'
import { Loader2 } from 'lucide-react'
import { useAction } from './bus'
import type { ActionId } from './actions/registry'

interface ActionButtonProps {
  actionId:  ActionId
  input:     unknown
  variant?:  'default' | 'primary' | 'danger' | 'ghost' | 'icon'
  size?:     'sm' | 'md'
  className?: string
  label?:    string
  onSuccess?: () => void
  onError?:  (err: unknown) => void
}

export function ActionButton({
  actionId, input, variant = 'default', size = 'sm',
  className, label, onSuccess, onError,
}: ActionButtonProps) {
  const { exec, loading, def } = useAction(actionId)
  const Icon = def.icon

  const cls = clsx(
    'btn inline-flex items-center gap-1.5',
    size === 'sm'   ? 'text-[9px] px-2 py-1'   : 'text-[11px] px-3 py-1.5',
    variant === 'primary' && 'btn-primary',
    variant === 'danger'  && 'btn-danger',
    variant === 'ghost'   && 'btn-ghost',
    variant === 'icon'    && 'p-1.5 aspect-square',
    loading               && 'opacity-60 cursor-wait',
    className,
  )

  return (
    <button
      className={cls}
      disabled={loading}
      title={def.description}
      onClick={() => exec(input as never, { onSuccess, onError })}
    >
      {loading
        ? <Loader2 size={size === 'sm' ? 10 : 12} className="animate-spin" />
        : <Icon   size={size === 'sm' ? 10 : 12} />
      }
      {variant !== 'icon' && (label ?? def.label)}
    </button>
  )
}
