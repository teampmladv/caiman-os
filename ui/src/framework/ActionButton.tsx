import React from 'react'
import { clsx } from 'clsx'
import { Loader2 } from 'lucide-react'
import { useAction } from './bus'
import type { ActionId } from './actions/registry'

interface ActionButtonProps {
  actionId:   ActionId
  input:      unknown
  variant?:   'default' | 'primary' | 'danger' | 'ghost' | 'icon'
  size?:      'sm' | 'md'
  className?: string
  label?:     string
  onSuccess?: () => void
  onError?:   (err: unknown) => void
}

export function ActionButton({
  actionId, input, variant = 'default', size = 'sm',
  className, label, onSuccess, onError,
}: ActionButtonProps) {
  const { exec, loading, def } = useAction(actionId)
  const Icon = def.icon

  return (
    <button
      className={clsx(
        'btn inline-flex items-center gap-1.5',
        size === 'sm' ? 'text-[9px] px-2 py-1' : 'text-[11px] px-3 py-1.5',
        variant === 'primary' && 'btn-primary',
        variant === 'danger'  && 'btn-danger',
        variant === 'ghost'   && 'btn-ghost',
        variant === 'icon'    && 'p-1.5 aspect-square',
        loading && 'opacity-60 cursor-wait',
        className,
      )}
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
